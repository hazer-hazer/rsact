# Async-agnostic tile output — research

**Date:** 2026-08-01
**Status:** Research only. No code written, no plan produced. Input for the WS6.4 tiling design session.
**Scope:** How rsact hands rendered pixels to a display when the transport may be blocking SPI, polled DMA, Embassy async, or an RTIC ISR — without the framework choosing a concurrency model.
**Relates to:** WS6.4 (framebuffer strip modes), WS6.5 (e-paper), WS6.6 (dirty-list walk). Builds on merged WS6.2/6.3a (damage-driven flush) and WS6.3b (fast `fill_solid`).

---

## 1. The question

Today the flush is a push: the core streams pixels into a sink.

```
Page::render(target)                          rsact-ui/src/page/mod.rs:675
  └─ FinishRender::finish_frame_regions(target, &damage)
       └─ renderer_output_regions(target, regions)   rsact-render/src/eg/renderer.rs:221
            └─ Framebuf::output_region(target, region)  rsact-render/src/eg/framebuf.rs:166
                 └─ target.draw(iterator_of_pixels)     rsact-render/src/output/mod.rs:9
```

The user-facing shape being considered was:

```rust
ui.render(|region| spi_display.draw_region(region));
```

For a DMA transport the callback would need to be `async`, and the core must stay unblocked during the transfer so it can render the next tile. The question: can this be async-agnostic — one core serving both sync and async transports — without generics over asyncness?

**Answer: yes, via sans-IO. The core performs no I/O and never suspends; it hands back owned tiles and the application performs the transfer.**

---

## 2. Why nothing else is genuinely agnostic

Rust has no effect polymorphism on stable. One function cannot be `async` for one caller and blocking for another. The Keyword Generics Initiative (`?async`) was chartered to fix exactly this and nothing has shipped. So if I/O lives *inside* the core, you must pick one of these:

| Approach | Agnostic? | Cost |
|---|---|---|
| `async fn` in `FinishRender` | No | Colors `finish_frame_regions` → `Page::render` → `UI::render` → app loop. Not dyn-safe without `alloc`. Blocking users need `block_on` + an executor dependency to draw a button. |
| Mirrored sync/async traits (`embedded-hal` / `embedded-hal-async` model) | No — duplicated | Two of everything, permanently. Feature flags are global, so one binary cannot drive a blocking e-paper *and* a DMA LCD. |
| `maybe-async-cfg` / macro-generated dual API | No — duplicated at build time | Same as above plus macro-obscured source and a doubled test matrix. |
| `nb::Result` poll-based | Partially | Executor-agnostic but busy-polling is the only sync story; does not compose with `.await` without a shim. |
| **Sans-IO (return owned tiles)** | **Yes** | Caller writes ~4 lines instead of 1. Core must become resumable (a cursor over the damage list). |

**Why sans-IO escapes coloring:** coloring propagates through *callers*, not *callees*. If the core calls the sink, the core is colored. If the core returns to the caller and the caller calls the sink, the core is colorless, and there is no mechanism by which the sink's color travels back up. Inverting the call direction is the only in-language escape available today.

Prior art: `quinn-proto`, `rustls::ConnectionCommon`, `httparse`, LVGL's draw-buffer + `lv_display_flush_ready()`.

---

## 3. The mechanism: three-state tile lifecycle

```
        core fills                app starts transfer        app signals done
Free ──────────────► Ready ──────────────────────► InFlight ─────────────────► Free
  ▲                                                                             │
  └─────────────────────────────────────────────────────────────────────────────┘
```

Three synchronous operations:

- **`next_tile() -> Option<Tile>`** — core renders (or copies) into a free buffer and moves it out. `None` means either *frame complete* or *no free buffer, release one first*. **These two must be distinguishable** or a missing `release` becomes a silent deadlock instead of a diagnosable error.
- **`release(Tile)`** — app returns the buffer once hardware is done. The only synchronization point in the entire design. Moral equivalent of `lv_display_flush_ready()`.
- **cursor state** — the core stores its position in the damage list so it can be re-entered between transfers. Without resumability sans-IO does not work: the core would have to re-derive what it already flushed.

### The same core, every transport

```rust
let mut flush = ui.begin_flush(buffers);   // sync, no executor, no I/O

// Blocking SPI:
while let Some(tile) = flush.next_tile(&mut ui) {
    spi.write_window(tile.rect, tile.bytes())?;
    flush.release(tile);
}

// Async — identical, plus one keyword the framework never observes:
while let Some(tile) = flush.next_tile(&mut ui) {
    spi.write_window(tile.rect, tile.bytes()).await?;
    flush.release(tile);
}

// DMA ping-pong — core renders tile N+1 while N is in flight:
let mut inflight = None;
while let Some(tile) = flush.next_tile(&mut ui) {
    let xfer = spi.write_dma(tile.rect, tile);      // tile MOVES into the transfer
    if let Some(prev) = inflight.replace(xfer) {
        flush.release(prev.wait().await);            // buffer returns to the pool
    }
}
```

No feature flag, no `block_on`, no duplicated trait, no generics over asyncness.

---

## 4. Ownership is the load-bearing part

`Tile` must be **owned/moved**, not a `&[u8]` borrowed from a framebuffer. Three independent reasons, none of which are about async:

1. **DMA soundness.** A DMA engine reading a buffer is not a thread the borrow checker can see. `embedded-dma`'s `ReadBuffer`/`WriteBuffer` are `unsafe` traits precisely because the only way to make this sound is to prove the buffer is stable and exclusively owned for the transfer's duration. Handing DMA a borrow the core can still write through is UB, and it manifests as tearing, not as a crash.
2. **Borrow across suspension.** A borrow of the framebuffer held across `.await` keeps `&mut ui` locked, making "render the next tile during the transfer" literally uncompilable. Moving the tile out releases the core immediately.
3. **`'static` requirements in real HALs.** Many HAL DMA APIs (`stm32f4xx-hal`, `nrf-hal`) demand `'static` buffers because the transfer object can outlive the calling scope. A tile borrowed from a UI struct is never `'static`; a tile from a user-owned static pool is.

**This is why one mechanism solves both problems.** "Async-agnostic" and "overlap compute with DMA" are the same underlying question — *who is allowed to touch this memory right now?* — and both are answered by making the transfer of permission an explicit synchronous API call rather than an implicit consequence of control flow.

---

## 5. One core, four execution models

The test of genuine agnosticism is whether the same synchronous core serves all of these without knowing which it is in:

| Model | Where `release` is called |
|---|---|
| Bare-metal blocking SPI | Immediately after `spi.write()` returns — degenerates to today's behaviour, zero overhead |
| Bare-metal DMA + polling | Main loop, after the transfer-complete flag; `WFI` between checks |
| Embassy / async | After `transfer.await` in the app's task; core is free during the await |
| RTIC | From the DMA-complete **ISR**, pushed to a `heapless::spsc` queue drained by the idle task — the core needs no critical section because it only ever runs in one priority context |

The RTIC row is the strongest evidence the design is right: an interrupt-driven completion path is expressible without the core knowing interrupts exist. An async-trait design cannot do this at all — it would need a waker plumbed into the ISR.

**E-paper (WS6.5) falls out free:** `release` is called when the BUSY pin deasserts. A multi-second panel refresh and a 2 ms DMA burst are the same protocol.

---

## 6. Decisions taken (maintainer, this session)

1. **Render directly into tiles**, double-buffered — one tile transferring while the next is drawn. Full-framebuffer double buffering remains available as an option.
2. **Buffers are user-provided**, not rsact-allocated.

### Consequences of (1)

Keep the *flush protocol* completely independent of how a tile gets filled. `next_tile`/`release` are identical whether the tile was rendered into or copied into. Buffer strategy is a property of the producer, not of the API — that is what lets both modes coexist without a second trait.

### Consequences of (2)

- The core cannot assume alignment, `'static`-ness, or cache coherency, and should not try to.
- **The ownership handshake gives the user exactly the right window for cache maintenance.** On Cortex-M7 or ESP32-S3 with D-cache, the buffer must be cleaned after CPU writes and before DMA reads — precisely the interval when the app holds the tile and the core does not. A borrow-based API would have no such window.
- Buffer *count* becomes a runtime property, not a framework knob. One buffer degenerates to sequential flush through the identical code path; two give ping-pong; N give deeper pipelining. The user picks the RAM/latency trade-off with no feature flag.

---

## 7. Contiguity: what "DMA-friendly" means, and why (1) mostly deletes the problem

**The display does not care about rect shape.** Every controller (ST7789, ILI9341, SSD1306, …) takes `CASET`/`RASET` to set an address window, then auto-wraps its internal write pointer at the window's right edge. A centered rect is one address-window command plus one linear byte stream.

**Memory is where it breaks — and only when the source is a full framebuffer.** Row-major storage puts pixel `(x,y)` at `(y·W + x)·bpp`, so a `Wr × Hr` rect is `Hr` separate runs separated by the framebuffer stride:

```
framebuffer (W wide)          rect rows in memory
┌──────────────────────┐      ▓▓▓▓░░░░░░░░░░░░░░░░   run 1
│      ┌──────┐        │      ░░░░░░░░▓▓▓▓░░░░░░░░   run 2   ← stride W·bpp apart
│      │ rect │        │      ░░░░░░░░░░░░░░░░▓▓▓▓   run 3
│      └──────┘        │      not one span → not one DMA
└──────────────────────┘
```

Most Cortex-M DMA is linear-increment only, no 2D stride. A strided rect therefore costs `Hr` transfers with `Hr` setups and completion IRQs, which for thin/tall damage dominates actual byte time. Escapes exist per-platform: ESP32 GDMA linked-list descriptors do scatter-gather; STM32 DMA2D (Chrom-ART) does 2D but memory-to-memory, so it compacts rather than feeds SPI. A **full-width strip is the only rect shape contiguous inside a full framebuffer** — that is all "DMA-friendly strip" meant.

**With direct-to-tile rendering this disappears.** The tile buffer holds exactly the sub-rect, in its own scan order, with the tile's own width as its stride. It is contiguous by construction, for any rect shape, anywhere on screen.

> **Damage rects do not need reshaping into strips. They need chunking to fit the buffer.** A rect taller than the buffer holds splits into row-bands of the same width; each band renders linearly into the tile and ships as one address-window + one DMA.

The strip constraint survives **only** in the optional zero-copy-from-full-framebuffer path. That path additionally requires the framebuffer to store pixels in **wire format** — SPI panels overwhelmingly want big-endian RGB565, while a native `u16` is little-endian on ARM. Byte-swapping at flush time is a copy, which puts you back on the tile path. If zero-copy is wanted, the framebuffer must store big-endian and every `set_pixel`/blend pays the swap instead.

**General principle worth reusing:** a constraint that looks like it belongs to the *data* (damage shape) often belongs to the *storage decision* (who owns the pixels). Choosing direct-to-tile moved contiguity from something damage must be reshaped to achieve into something the buffer guarantees for free. Check whether the same reframing kills other constraints on the WS6.4 list.

### Constraint that survives regardless

**Some SPI peripherals cap a single DMA transfer length** — nRF52's SPIM EasyDMA `MAXCNT` is the classic example, narrow on some parts in the family. This is a hard ceiling on tile byte-size independent of available RAM. Tile geometry should derive from `min(buffer_capacity, peripheral_max_transfer)`, with the latter supplied by the app since only the app knows the part.

---

## 8. Frame coherence: defer + coalesce, do not abort

The dilemma posed was *abort the in-flight frame* vs *sync tiles across frames*. There is a third option that gets the benefit of both:

> **Defer, and coalesce damage by union.**

The in-flight frame runs to completion. New damage produced during the flush accumulates into the *next* frame's damage set. The core does not begin a new render pass until the current flush drains. Only ever one frame in flight, so generations never mix — the property abort was wanted for, without abort.

### Why this cannot starve

Starvation is a property of *abort*, not of deferral. Abort discards completed work, so under sustained load you can loop forever without ever presenting. Deferral **merges** work instead of queueing it — N frames of changes during one flush window collapse into one union'd damage set, bounded by screen area. Therefore:

- Forward progress is guaranteed; every flush completes.
- The pending set cannot grow — it is a union, not a list. This is what makes deferral safe under arbitrary load, and the reason to prefer union-accumulation over a frame queue.
- Latency is bounded: worst case a change appears after (remaining flush + one full flush).
- Effective frame rate becomes flush-bound, which is physically true — you cannot present faster than the bus carries pixels.

The accumulator already exists: the damage sink at `rsact-ui/src/el/render.rs:39`. The change is that it must survive across a flush boundary rather than being cleared per pass as `rsact-ui/src/page/mod.rs:700` does today.

### The one case where discarding is correct

Discarding pending tiles is safe **iff the new damage set covers every not-yet-transferred tile**. Page navigation and full-screen invalidation satisfy this trivially, and there, continuing to ship old-page tiles is pure wasted bus time. Checkable predicate, not a heuristic:

```
if new_damage ⊇ every pending tile:  drain the in-flight tile, discard the rest
else:                                 defer, union the damage
```

Let the in-flight tile finish rather than aborting the DMA mid-stream. It costs microseconds-to-milliseconds, whereas aborting leaves the panel's write pointer at an unknown position inside its address window.

### Explicitly rejected

Re-rendering a not-yet-started tile with newer state to "catch up". Tempting because the tile has not been touched, but it is exactly the generation-mixing to be avoided — a value and its label repainting one frame apart is a visible artifact, not a theoretical one.

---

## 9. Constraints checklist for the tiling design

- [ ] DMA needs contiguous bytes; `output_region` (`framebuf.rs:166`) currently fans out per-point and can never feed DMA. `draw_buffer` (`framebuf.rs:198`) is the right seam but exposes only the whole buffer.
- [ ] Tile geometry bounded by `min(buffer_capacity, peripheral_max_transfer)`.
- [ ] Wire format (endianness) decided per mode: tiles can convert while filling; zero-copy cannot.
- [ ] 1bpp/mono tiles byte-aligned in the panel's scan direction — otherwise the partial head/tail-word problem solved render-side in WS6.3b reappears at *transfer* boundaries where read-modify-write is not available.
- [ ] Alignment + cache maintenance are the app's responsibility; the API must leave a window for them (the handshake does).
- [ ] **Clip exactness at tile seams.** Direct-to-tile renders the widget tree once per tile; anti-aliased text straddling a seam must produce identical pixels whether rendered in one pass or two. This is the classic partial-render bug class. The WS6.9 golden harness is well-suited: a strip-mode golden must byte-match the full-frame golden.
- [ ] Traversal cost: direct-to-tile visits the tree once per tile. Existing damage/probe machinery provides per-tile culling; measure it.
- [ ] `next_tile() -> None` must distinguish *frame done* from *no free buffer*.

---

## 10. Prior art to read

- **LVGL v9 display driver** — draw buffers + `lv_display_flush_ready()`. Closest analogue; their partial/direct/full render-mode taxonomy is the WS6.4 fork.
- **`quinn-proto`** — cleanest sans-IO in Rust. `poll_transmit` returns owned datagrams; the async `quinn` crate is a thin shell. Proof the pattern scales to a complex state machine.
- **`rustls::ConnectionCommon`** — same shape, notable for serving blocking, tokio, and embedded users from one core.
- **`embedded-dma`** — the safety contracts on `ReadBuffer` are the formal statement of §4.
- **`mipidsi`, `embedded-graphics-framebuf`** — address-window mechanics, wire-format conversion, and where existing drivers assume a blocking `write`.

---

## 11. Open questions for the tiling session

1. Does damage get chunked into buffer-sized row-bands per rect, or merged into a tile *grid* first (fewer, larger, more wasteful transfers vs. more, smaller, exact ones)?
2. How is the full-framebuffer mode exposed alongside direct-to-tile — same entry point with a different producer, or a separate constructor?
3. Does `begin_flush` take the buffers by value each frame, or does the core hold a pool registered once at UI construction?
4. Is the deferral gate a plain flag on `Page`, or does the frame carry a generation counter (needed only if pipelining across frames is ever revisited)?

Points 1–4 of §12 below are independent of all of these, which is the property wanted when deciding transport architecture ahead of tiling.

---

## 12. Summary

1. **Sans-IO core** — returns owned tiles, performs no I/O, never suspends. Async-agnostic because the caller owns the I/O call and therefore the color.
2. **Owned-tile handshake** (`next_tile` / `release`) — one protocol serving blocking SPI, polled DMA, Embassy, and RTIC-ISR completion, plus the app's cache-maintenance window.
3. **Direct-to-tile rendering** makes contiguity structural; damage rects chunk into row-bands sized to the buffer. Strips matter only for the optional zero-copy-full-framebuffer mode, which additionally requires wire-format storage.
4. **Defer + union-coalesce** for frame coherence, with superset-discard as the only abort case. Bounded memory, bounded latency, no starvation, no mixed generations.

`FinishRender` need not be removed: it becomes the convenience layer for the "I have a `DrawTarget`, just blit it" case (simulator, `rsact-render/src/eg/output.rs` blanket impl), re-expressed as "drive the cursor to completion synchronously". Nothing at `rsact-ui/src/ui.rs:251` breaks.
