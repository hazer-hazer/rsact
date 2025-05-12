```mermaid
graph TD

signal4294967297(" signal: u32 (clean)")




signal4294967298(" signal: alloc::vec::Vec<rsact_ui::event::message::UiMessage<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (clean)")




memo4294967299([" memo: u32 (dirty)"])




signal4294967300(" signal: alloc::string::String (clean)")




signal4294967301(" signal: std::time::Instant (clean)")




signal4294967302(" signal: bool (clean)")

effect4294967312 ===o |sub|signal4294967302
effect4294967312[[" effect: () (clean)"]]


signal4294967302 ===o |source|effect4294967312


effect4294967312 --> |clean|effect4294967312




signal4294967303(" signal: rsact_ui::value::RangeU8<0, u8::MAX> (clean)")




signal4294967304(" signal: rsact_ui::layout::Layout (clean)")




memoChain4294967305(((" memoChain: rsact_ui::widget::bar::BarStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967306([" memo: alloc::string::String (dirty)"])




memoChain4294967307(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967308(" signal: rsact_ui::layout::Layout (clean)")

effect4294967598 ===o |sub|signal4294967308
effect4294967598[[" effect: () (clean)"]]


signal4294967308 ===o |source|effect4294967598

signal4294967309 ===o |source|effect4294967598
signal4294967309(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967598 ===o |sub|signal4294967309



signal4294967309 --> |dirten|signal4294967309


effect4294967598 --> |clean|effect4294967598





memo4294967310([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967311(" 'Layout tree' signal: rsact_ui::layout::Layout (clean)")



signal4294967311 --> |clean|signal4294967311


signal4294967313(" signal: rsact_ui::font::FontSize (dirty)")



signal4294967313 --> |dirten|signal4294967313

signal4294967314(" signal: rsact_ui::layout::Layout (clean)")




memoChain4294967315(((" memoChain: rsact_ui::widget::icon::IconStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967316([" memo: alloc::string::String (dirty)"])




memoChain4294967317(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967318(" signal: rsact_ui::layout::Layout (clean)")

effect4294967632 ===o |sub|signal4294967318
effect4294967632[[" effect: () (clean)"]]


signal4294967318 ===o |source|effect4294967632

signal4294967319 ===o |source|effect4294967632
signal4294967319(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967632 ===o |sub|signal4294967319



signal4294967319 --> |dirten|signal4294967319


effect4294967632 --> |clean|effect4294967632





memo4294967320([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967321(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967321
effect4294967631[[" effect: () (clean)"]]


signal4294967321 ===o |source|effect4294967631

signal4294967323 ===o |source|effect4294967631
signal4294967323(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967323



signal4294967323 --> |clean|signal4294967323

signal4294967327 ===o |source|effect4294967631
signal4294967327(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967327




signal4294967329 ===o |source|effect4294967631
signal4294967329(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967329



signal4294967329 --> |clean|signal4294967329

signal4294967333 ===o |source|effect4294967631
signal4294967333(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967333




signal4294967335 ===o |source|effect4294967631
signal4294967335(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967335



signal4294967335 --> |clean|signal4294967335

signal4294967339 ===o |source|effect4294967631
signal4294967339(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967339




signal4294967341 ===o |source|effect4294967631
signal4294967341(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967341



signal4294967341 --> |clean|signal4294967341

signal4294967345 ===o |source|effect4294967631
signal4294967345(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967345




signal4294967347 ===o |source|effect4294967631
signal4294967347(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967347



signal4294967347 --> |clean|signal4294967347

signal4294967351 ===o |source|effect4294967631
signal4294967351(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967351




signal4294967353 ===o |source|effect4294967631
signal4294967353(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967353



signal4294967353 --> |clean|signal4294967353

signal4294967357 ===o |source|effect4294967631
signal4294967357(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967357




signal4294967359 ===o |source|effect4294967631
signal4294967359(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967359



signal4294967359 --> |clean|signal4294967359

signal4294967363 ===o |source|effect4294967631
signal4294967363(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967363




signal4294967365 ===o |source|effect4294967631
signal4294967365(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967365



signal4294967365 --> |clean|signal4294967365

signal4294967369 ===o |source|effect4294967631
signal4294967369(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967369




signal4294967371 ===o |source|effect4294967631
signal4294967371(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967371



signal4294967371 --> |clean|signal4294967371

signal4294967375 ===o |source|effect4294967631
signal4294967375(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967375




signal4294967377 ===o |source|effect4294967631
signal4294967377(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967377



signal4294967377 --> |clean|signal4294967377

signal4294967381 ===o |source|effect4294967631
signal4294967381(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967381




signal4294967383 ===o |source|effect4294967631
signal4294967383(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967383



signal4294967383 --> |clean|signal4294967383

signal4294967387 ===o |source|effect4294967631
signal4294967387(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967387




signal4294967389 ===o |source|effect4294967631
signal4294967389(" signal: rsact_ui::layout::Layout (clean)")

effect4294967631 ===o |sub|signal4294967389



signal4294967389 --> |clean|signal4294967389

signal4294967391 ===o |source|effect4294967631
signal4294967391(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967631 ===o |sub|signal4294967391



signal4294967391 --> |dirten|signal4294967391


effect4294967631 --> |clean|effect4294967631



signal4294967321 --> |clean|signal4294967321

signal4294967322(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967324(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967325([" memo: alloc::string::String (dirty)"])




memoChain4294967326(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967328(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967330(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967331([" memo: alloc::string::String (dirty)"])




memoChain4294967332(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967334(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967336(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967337([" memo: alloc::string::String (dirty)"])




memoChain4294967338(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967340(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967342(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967343([" memo: alloc::string::String (dirty)"])




memoChain4294967344(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967346(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967348(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967349([" memo: alloc::string::String (dirty)"])




memoChain4294967350(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967352(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967354(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967355([" memo: alloc::string::String (dirty)"])




memoChain4294967356(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967358(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967360(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967361([" memo: alloc::string::String (dirty)"])




memoChain4294967362(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967364(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967366(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967367([" memo: alloc::string::String (dirty)"])




memoChain4294967368(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967370(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967372(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967373([" memo: alloc::string::String (dirty)"])




memoChain4294967374(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967376(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967378(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967379([" memo: alloc::string::String (dirty)"])




memoChain4294967380(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967382(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967384(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967385([" memo: alloc::string::String (dirty)"])




memoChain4294967386(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967388(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967390(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967392([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967393(" signal: rsact_ui::layout::Layout (clean)")



signal4294967393 --> |clean|signal4294967393

signal4294967394(" signal: rsact_ui::widget::scrollable::ScrollableState (clean)")




signal4294967395(" 'Layout tree' signal: rsact_ui::layout::Layout (clean)")



signal4294967395 --> |clean|signal4294967395

memoChain4294967396(((" memoChain: rsact_ui::widget::scrollable::ScrollableStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967397(" signal: embedded_graphics_core::geometry::point::Point (clean)")




signal4294967398(" signal: i32 (clean)")




signal4294967399(" signal: bool (clean)")

effect4294967401 ===o |sub|signal4294967399
effect4294967401[[" effect: () (clean)"]]


signal4294967399 ===o |source|effect4294967401


effect4294967401 --> |clean|effect4294967401




signal4294967400(" signal: std::time::Instant (clean)")





signal4294967402(" signal: rsact_ui::font::FontSize (dirty)")



signal4294967402 --> |dirten|signal4294967402

signal4294967403(" signal: rsact_ui::layout::Layout (clean)")




memoChain4294967404(((" memoChain: rsact_ui::widget::icon::IconStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967405([" memo: alloc::string::String (dirty)"])




memoChain4294967406(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967407(" signal: rsact_ui::layout::Layout (clean)")

effect4294967608 ===o |sub|signal4294967407
effect4294967608[[" effect: () (clean)"]]


signal4294967407 ===o |source|effect4294967608

signal4294967408 ===o |source|effect4294967608
signal4294967408(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967608 ===o |sub|signal4294967408



signal4294967408 --> |dirten|signal4294967408


effect4294967608 --> |clean|effect4294967608





memo4294967409([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967410(" signal: rsact_ui::layout::Layout (clean)")

effect4294967607 ===o |sub|signal4294967410
effect4294967607[[" effect: () (clean)"]]


signal4294967410 ===o |source|effect4294967607

signal4294967412 ===o |source|effect4294967607
signal4294967412(" signal: rsact_ui::layout::Layout (clean)")

effect4294967607 ===o |sub|signal4294967412



signal4294967412 --> |clean|signal4294967412

signal4294967416 ===o |source|effect4294967607
signal4294967416(" signal: rsact_ui::layout::Layout (clean)")

effect4294967607 ===o |sub|signal4294967416




signal4294967419 ===o |source|effect4294967607
signal4294967419(" signal: rsact_ui::layout::Layout (clean)")

effect4294967607 ===o |sub|signal4294967419




signal4294967421 ===o |source|effect4294967607
signal4294967421(" signal: rsact_ui::layout::Layout (clean)")

effect4294967607 ===o |sub|signal4294967421



signal4294967421 --> |clean|signal4294967421

signal4294967423 ===o |source|effect4294967607
signal4294967423(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967607 ===o |sub|signal4294967423



signal4294967423 --> |dirten|signal4294967423


effect4294967607 --> |clean|effect4294967607



signal4294967410 --> |clean|signal4294967410

signal4294967411(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967413(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967414([" memo: alloc::string::String (dirty)"])




memoChain4294967415(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967417([" memo: alloc::string::String (dirty)"])




memoChain4294967418(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967420(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967422(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967424([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967425(" signal: rsact_ui::layout::Layout (clean)")

effect4294967606 ===o |sub|signal4294967425
effect4294967606[[" effect: () (clean)"]]


signal4294967425 ===o |source|effect4294967606

signal4294967446 ===o |source|effect4294967606
signal4294967446(" signal: rsact_ui::layout::Layout (clean)")

effect4294967606 ===o |sub|signal4294967446



signal4294967446 --> |clean|signal4294967446

signal4294967464 ===o |source|effect4294967606
signal4294967464(" signal: rsact_ui::layout::Layout (clean)")

effect4294967606 ===o |sub|signal4294967464



signal4294967464 --> |clean|signal4294967464

signal4294967473 ===o |source|effect4294967606
signal4294967473(" signal: rsact_ui::layout::Layout (clean)")

effect4294967606 ===o |sub|signal4294967473



signal4294967473 --> |clean|signal4294967473

signal4294967474 ===o |source|effect4294967606
signal4294967474(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967606 ===o |sub|signal4294967474



signal4294967474 --> |dirten|signal4294967474


effect4294967606 --> |clean|effect4294967606



signal4294967425 --> |clean|signal4294967425

memo4294967426([" memo: alloc::string::String (dirty)"])




memoChain4294967427(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967428(" signal: rsact_ui::layout::Layout (clean)")

effect4294967609 ===o |sub|signal4294967428
effect4294967609[[" effect: () (clean)"]]


signal4294967428 ===o |source|effect4294967609

signal4294967430 ===o |source|effect4294967609
signal4294967430(" signal: rsact_ui::layout::Layout (clean)")

effect4294967609 ===o |sub|signal4294967430



signal4294967430 --> |clean|signal4294967430

signal4294967434 ===o |source|effect4294967609
signal4294967434(" signal: rsact_ui::layout::Layout (clean)")

effect4294967609 ===o |sub|signal4294967434




signal4294967436 ===o |source|effect4294967609
signal4294967436(" signal: rsact_ui::layout::Layout (clean)")

effect4294967609 ===o |sub|signal4294967436



signal4294967436 --> |clean|signal4294967436

signal4294967440 ===o |source|effect4294967609
signal4294967440(" signal: rsact_ui::layout::Layout (clean)")

effect4294967609 ===o |sub|signal4294967440




signal4294967442 ===o |source|effect4294967609
signal4294967442(" signal: rsact_ui::layout::Layout (clean)")

effect4294967609 ===o |sub|signal4294967442



signal4294967442 --> |clean|signal4294967442

signal4294967444 ===o |source|effect4294967609
signal4294967444(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967609 ===o |sub|signal4294967444



signal4294967444 --> |dirten|signal4294967444


effect4294967609 --> |clean|effect4294967609




signal4294967429(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967431(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967432([" memo: alloc::string::String (dirty)"])




memoChain4294967433(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967435(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967437(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967438([" memo: alloc::string::String (dirty)"])




memoChain4294967439(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967441(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967443(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967445([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])





memo4294967447([" memo: alloc::string::String (dirty)"])




memoChain4294967448(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967449(" signal: rsact_ui::layout::Layout (clean)")

effect4294967610 ===o |sub|signal4294967449
effect4294967610[[" effect: () (clean)"]]


signal4294967449 ===o |source|effect4294967610

signal4294967451 ===o |source|effect4294967610
signal4294967451(" signal: rsact_ui::layout::Layout (clean)")

effect4294967610 ===o |sub|signal4294967451



signal4294967451 --> |clean|signal4294967451

signal4294967455 ===o |source|effect4294967610
signal4294967455(" signal: rsact_ui::layout::Layout (clean)")

effect4294967610 ===o |sub|signal4294967455




signal4294967458 ===o |source|effect4294967610
signal4294967458(" signal: rsact_ui::layout::Layout (clean)")

effect4294967610 ===o |sub|signal4294967458




signal4294967460 ===o |source|effect4294967610
signal4294967460(" signal: rsact_ui::layout::Layout (clean)")

effect4294967610 ===o |sub|signal4294967460



signal4294967460 --> |clean|signal4294967460

signal4294967462 ===o |source|effect4294967610
signal4294967462(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967610 ===o |sub|signal4294967462



signal4294967462 --> |dirten|signal4294967462


effect4294967610 --> |clean|effect4294967610




signal4294967450(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967452(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967453([" memo: alloc::string::String (dirty)"])




memoChain4294967454(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967456([" memo: alloc::string::String (dirty)"])




memoChain4294967457(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967459(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967461(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967463([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])





memo4294967465([" memo: alloc::string::String (dirty)"])




memoChain4294967466(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967467(" signal: rsact_ui::layout::Layout (clean)")

effect4294967611 ===o |sub|signal4294967467
effect4294967611[[" effect: () (clean)"]]


signal4294967467 ===o |source|effect4294967611

signal4294967469 ===o |source|effect4294967611
signal4294967469(" signal: rsact_ui::layout::Layout (clean)")

effect4294967611 ===o |sub|signal4294967469



signal4294967469 --> |clean|signal4294967469

signal4294967471 ===o |source|effect4294967611
signal4294967471(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967611 ===o |sub|signal4294967471



signal4294967471 --> |dirten|signal4294967471


effect4294967611 --> |clean|effect4294967611




signal4294967468(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967470(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967472([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])






memo4294967475([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967476(" 'Layout tree' signal: rsact_ui::layout::Layout (clean)")



signal4294967476 --> |clean|signal4294967476

signal4294967477(" signal: rsact_ui::value::RangeU8<0, 110> (clean)")




signal4294967478(" signal: rsact_ui::value::RangeU8<0, 250> (clean)")




signal4294967479(" signal: std::time::Instant (clean)")




signal4294967480(" signal: bool (clean)")

effect4294967481 ===o |sub|signal4294967480
effect4294967481[[" effect: () (clean)"]]


signal4294967480 ===o |source|effect4294967481


effect4294967481 --> |clean|effect4294967481





signal4294967482(" signal: rsact_ui::font::FontSize (dirty)")



signal4294967482 --> |dirten|signal4294967482

signal4294967483(" signal: rsact_ui::layout::Layout (clean)")




memoChain4294967484(((" memoChain: rsact_ui::widget::icon::IconStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967485([" memo: alloc::string::String (dirty)"])




memoChain4294967486(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967487(" signal: rsact_ui::layout::Layout (clean)")

effect4294967621 ===o |sub|signal4294967487
effect4294967621[[" effect: () (clean)"]]


signal4294967487 ===o |source|effect4294967621

signal4294967488 ===o |source|effect4294967621
signal4294967488(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967621 ===o |sub|signal4294967488



signal4294967488 --> |dirten|signal4294967488


effect4294967621 --> |clean|effect4294967621





memo4294967489([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967490(" signal: rsact_ui::layout::Layout (clean)")

effect4294967620 ===o |sub|signal4294967490
effect4294967620[[" effect: () (clean)"]]


signal4294967490 ===o |source|effect4294967620

signal4294967492 ===o |source|effect4294967620
signal4294967492(" signal: rsact_ui::layout::Layout (clean)")

effect4294967620 ===o |sub|signal4294967492



signal4294967492 --> |clean|signal4294967492

signal4294967496 ===o |source|effect4294967620
signal4294967496(" signal: rsact_ui::layout::Layout (clean)")

effect4294967620 ===o |sub|signal4294967496




signal4294967498 ===o |source|effect4294967620
signal4294967498(" signal: rsact_ui::layout::Layout (clean)")

effect4294967620 ===o |sub|signal4294967498



signal4294967498 --> |clean|signal4294967498

signal4294967500 ===o |source|effect4294967620
signal4294967500(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967620 ===o |sub|signal4294967500



signal4294967500 --> |dirten|signal4294967500


effect4294967620 --> |clean|effect4294967620



signal4294967490 --> |clean|signal4294967490

signal4294967491(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967493(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967494([" memo: alloc::string::String (dirty)"])




memoChain4294967495(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





signal4294967497(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967499(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memo4294967501([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967502(" signal: rsact_ui::layout::Layout (clean)")

effect4294967619 ===o |sub|signal4294967502
effect4294967619[[" effect: () (clean)"]]


signal4294967502 ===o |source|effect4294967619

signal4294967514 ===o |source|effect4294967619
signal4294967514(" signal: rsact_ui::layout::Layout (clean)")

effect4294967619 ===o |sub|signal4294967514



signal4294967514 --> |clean|signal4294967514

signal4294967526 ===o |source|effect4294967619
signal4294967526(" signal: rsact_ui::layout::Layout (clean)")

effect4294967619 ===o |sub|signal4294967526



signal4294967526 --> |clean|signal4294967526

signal4294967527 ===o |source|effect4294967619
signal4294967527(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967619 ===o |sub|signal4294967527



signal4294967527 --> |dirten|signal4294967527


effect4294967619 --> |clean|effect4294967619



signal4294967502 --> |clean|signal4294967502

memo4294967503([" memo: alloc::string::String (dirty)"])




memoChain4294967504(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967505(" signal: rsact_ui::layout::Layout (clean)")

effect4294967622 ===o |sub|signal4294967505
effect4294967622[[" effect: () (clean)"]]


signal4294967505 ===o |source|effect4294967622

signal4294967511 ===o |source|effect4294967622
signal4294967511(" signal: rsact_ui::layout::Layout (clean)")

effect4294967622 ===o |sub|signal4294967511




signal4294967512 ===o |source|effect4294967622
signal4294967512(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967622 ===o |sub|signal4294967512



signal4294967512 --> |dirten|signal4294967512


effect4294967622 --> |clean|effect4294967622




signal4294967506(" signal: rsact_ui::layout::Layout (clean)")




signal4294967507(" signal: rsact_ui::widget::knob::KnobState (clean)")




memoChain4294967508(((" memoChain: rsact_ui::widget::knob::KnobStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967509([" memo: alloc::string::String (dirty)"])




memoChain4294967510(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))






memo4294967513([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])





memo4294967515([" memo: alloc::string::String (dirty)"])




memoChain4294967516(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




signal4294967517(" signal: rsact_ui::layout::Layout (clean)")

effect4294967623 ===o |sub|signal4294967517
effect4294967623[[" effect: () (clean)"]]


signal4294967517 ===o |source|effect4294967623

signal4294967523 ===o |source|effect4294967623
signal4294967523(" signal: rsact_ui::layout::Layout (clean)")

effect4294967623 ===o |sub|signal4294967523




signal4294967524 ===o |source|effect4294967623
signal4294967524(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (dirty)")

effect4294967623 ===o |sub|signal4294967524



signal4294967524 --> |dirten|signal4294967524


effect4294967623 --> |clean|effect4294967623




signal4294967518(" signal: rsact_ui::layout::Layout (clean)")




signal4294967519(" signal: rsact_ui::widget::knob::KnobState (clean)")




memoChain4294967520(((" memoChain: rsact_ui::widget::knob::KnobStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))




memo4294967521([" memo: alloc::string::String (dirty)"])




memoChain4294967522(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))






memo4294967525([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])






memo4294967528([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (dirty)"])




signal4294967529(" 'Layout tree' signal: rsact_ui::layout::Layout (clean)")



signal4294967529 --> |clean|signal4294967529

signal4294967530(" signal: rsact_ui::font::FontSize (clean)")

memo4294967588 ===o |sub|signal4294967530
memo4294967588([" 'Layout model' memo: rsact_ui::layout::LayoutModel (clean)"])

observer4294967641 ===o |sub|memo4294967588
observer4294967641{" observer (clean)"}


memo4294967588 ===o |source|observer4294967641

signal4294967589 ===o |source|observer4294967641
signal4294967589(" 'Page style' signal: rsact_ui::page::PageStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")

observer4294967641 ===o |sub|signal4294967589




observer4294967642 ===o |source|observer4294967641
observer4294967642{" observer (clean)"}

observer4294967641 ===o |sub|observer4294967642

observer4294967642 ===o |sub|observer4294967642


signal4294967530 ===o |source|observer4294967642

memoChain4294967532 ===o |source|observer4294967642
memoChain4294967532(((" memoChain: rsact_ui::widget::icon::IconStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")))

observer4294967642 ===o |sub|memoChain4294967532


memo4294967576 ===o |source|memoChain4294967532
memo4294967576([" memo: rsact_ui::style::NullStyler (clean)"])

memoChain4294967532 ===o |sub|memo4294967576

memoChain4294967541 ===o |sub|memo4294967576
memoChain4294967541(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")))


signal4294967539 ===o |source|memoChain4294967541
signal4294967539(" signal: rsact_ui::widget::button::ButtonState (clean)")

memoChain4294967541 ===o |sub|signal4294967539




memo4294967576 ===o |source|memoChain4294967541


memoChain4294967541 --> |clean|memoChain4294967541

memoChain4294967571 ===o |sub|memo4294967576
memoChain4294967571(((" memoChain: rsact_ui::widget::scrollable::ScrollableStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")))


signal4294967569 ===o |source|memoChain4294967571
signal4294967569(" signal: rsact_ui::widget::scrollable::ScrollableState (clean)")

memoChain4294967571 ===o |sub|signal4294967569




memo4294967576 ===o |source|memoChain4294967571


memoChain4294967571 --> |clean|memoChain4294967571



memo4294967576 --> |clean|memo4294967576


memoChain4294967532 --> |clean|memoChain4294967532

signal4294967536 ===o |source|observer4294967642
signal4294967536(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (clean)")

memo4294967537 ===o |sub|signal4294967536
memo4294967537([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (clean)"])

memo4294967588 ===o |sub|memo4294967537


signal4294967536 ===o |source|memo4294967537


memo4294967537 --> |clean|memo4294967537

effect4294967580 ===o |sub|signal4294967536
effect4294967580[[" effect: () (dirty)"]]


signal4294967535 ===o |source|effect4294967580
signal4294967535(" signal: rsact_ui::layout::Layout (clean)")

effect4294967580 ===o |sub|signal4294967535

memo4294967588 ===o |sub|signal4294967535




signal4294967536 ===o |source|effect4294967580


signal4294967536 ===> |dirten|effect4294967580



signal4294967536 --> |clean|signal4294967536

signal4294967540 ===o |source|observer4294967642
signal4294967540(" signal: rsact_ui::layout::Layout (clean)")

effect4294967579 ===o |sub|signal4294967540
effect4294967579[[" effect: () (dirty)"]]


signal4294967538 ===o |source|effect4294967579
signal4294967538(" signal: rsact_ui::layout::Layout (clean)")

effect4294967579 ===o |sub|signal4294967538

memo4294967588 ===o |sub|signal4294967538



signal4294967538 --> |clean|signal4294967538

signal4294967540 ===o |source|effect4294967579

signal4294967550 ===o |source|effect4294967579
signal4294967550(" signal: rsact_ui::layout::Layout (clean)")

effect4294967579 ===o |sub|signal4294967550

memo4294967588 ===o |sub|signal4294967550



signal4294967550 --> |clean|signal4294967550

signal4294967552 ===o |source|effect4294967579
signal4294967552(" signal: rsact_ui::layout::Layout (clean)")

effect4294967579 ===o |sub|signal4294967552

memo4294967588 ===o |sub|signal4294967552



signal4294967552 --> |clean|signal4294967552

signal4294967562 ===o |source|effect4294967579
signal4294967562(" signal: rsact_ui::layout::Layout (clean)")

effect4294967579 ===o |sub|signal4294967562

memo4294967588 ===o |sub|signal4294967562



signal4294967562 --> |clean|signal4294967562

signal4294967564 ===o |source|effect4294967579
signal4294967564(" signal: rsact_ui::layout::Layout (clean)")

effect4294967579 ===o |sub|signal4294967564

memo4294967588 ===o |sub|signal4294967564



signal4294967564 --> |clean|signal4294967564

signal4294967566 ===o |source|effect4294967579

signal4294967566(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (clean)")

memo4294967567 ===o |sub|signal4294967566
memo4294967567([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (clean)"])

memo4294967588 ===o |sub|memo4294967567


signal4294967566 ===o |source|memo4294967567


memo4294967567 --> |clean|memo4294967567

effect4294967579 ===o |sub|signal4294967566

memo4294967584 ===o |sub|signal4294967566
memo4294967584([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (clean)"])

memo4294967587 ===o |sub|memo4294967584
memo4294967587([" 'Focusable' memo: alloc::vec::Vec<rsact_ui::el::ElId> (clean)"])


memo4294967583 ===o |source|memo4294967587
memo4294967583([" memo: rsact_ui::widget::Meta (clean)"])

memo4294967587 ===o |sub|memo4294967583



memo4294967583 --> |clean|memo4294967583

memo4294967584 ===o |source|memo4294967587

memo4294967585 ===o |source|memo4294967587
memo4294967585([" memo: rsact_ui::widget::Meta (clean)"])

memo4294967587 ===o |sub|memo4294967585



memo4294967585 --> |clean|memo4294967585

memo4294967586 ===o |source|memo4294967587
memo4294967586([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (clean)"])

memo4294967587 ===o |sub|memo4294967586



memo4294967586 --> |clean|memo4294967586

memo4294967591 ===o |source|memo4294967587
memo4294967591([" memo: rsact_ui::widget::Meta (clean)"])

memo4294967587 ===o |sub|memo4294967591



memo4294967591 --> |clean|memo4294967591

memo4294967592 ===o |source|memo4294967587
memo4294967592([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (clean)"])

memo4294967587 ===o |sub|memo4294967592



memo4294967592 --> |clean|memo4294967592

memo4294967593 ===o |source|memo4294967587
memo4294967593([" memo: rsact_ui::widget::Meta (clean)"])

memo4294967587 ===o |sub|memo4294967593



memo4294967593 --> |clean|memo4294967593

memo4294967594 ===o |source|memo4294967587
memo4294967594([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (clean)"])

memo4294967587 ===o |sub|memo4294967594



memo4294967594 --> |clean|memo4294967594

memo4294967595 ===o |source|memo4294967587
memo4294967595([" memo: rsact_ui::widget::Meta (clean)"])

memo4294967587 ===o |sub|memo4294967595



memo4294967595 --> |clean|memo4294967595

memo4294967596 ===o |source|memo4294967587
memo4294967596([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (clean)"])

memo4294967587 ===o |sub|memo4294967596



memo4294967596 --> |clean|memo4294967596


memo4294967587 --> |clean|memo4294967587


signal4294967566 ===o |source|memo4294967584


memo4294967584 --> |clean|memo4294967584



signal4294967566 --> |clean|signal4294967566

signal4294967566 ===> |dirten|effect4294967579

memo4294967588 ===o |sub|signal4294967540



signal4294967540 --> |clean|signal4294967540

memoChain4294967541 ===o |source|observer4294967642

signal4294967566 ===o |source|observer4294967642

signal4294967569 ===o |source|observer4294967642

signal4294967570 ===o |source|observer4294967642
signal4294967570(" 'Layout tree' signal: rsact_ui::layout::Layout (clean)")

memo4294967588 ===o |sub|signal4294967570



signal4294967570 --> |clean|signal4294967570

memoChain4294967571 ===o |source|observer4294967642

memo4294967573 ===o |source|observer4294967642
memo4294967573([" 'Viewport' memo: rsact_ui::layout::size::Size (clean)"])

memo4294967588 ===o |sub|memo4294967573

observer4294967642 ===o |sub|memo4294967573


memo4294967572 ===o |source|memo4294967573
memo4294967572([" memo: embedded_graphics_core::geometry::size::Size (clean)"])

memo4294967573 ===o |sub|memo4294967572



memo4294967572 --> |clean|memo4294967572


memo4294967573 --> |clean|memo4294967573

signal4294967578 ===o |source|observer4294967642
signal4294967578(" 'Page state' signal: rsact_ui::widget::ctx::PageState<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>> (clean)")

observer4294967642 ===o |sub|signal4294967578



signal4294967578 --> |clean|signal4294967578

signal4294967590 ===o |source|observer4294967642
signal4294967590(" 'Force redraw' signal: () (dirty)")

observer4294967642 ===o |sub|signal4294967590



signal4294967590 --> |dirten|signal4294967590

observer4294967642 ===o |source|observer4294967642


observer4294967642 --> |clean|observer4294967642


observer4294967641 --> |clean|observer4294967641


signal4294967530 ===o |source|memo4294967588

signal4294967531 ===o |source|memo4294967588
signal4294967531(" signal: rsact_ui::layout::Layout (clean)")

memo4294967588 ===o |sub|signal4294967531




memo4294967533 ===o |source|memo4294967588
memo4294967533([" memo: alloc::string::String (clean)"])

memo4294967588 ===o |sub|memo4294967533



memo4294967533 --> |clean|memo4294967533

signal4294967535 ===o |source|memo4294967588

memo4294967537 ===o |source|memo4294967588

signal4294967538 ===o |source|memo4294967588

signal4294967540 ===o |source|memo4294967588

signal4294967542 ===o |source|memo4294967588
signal4294967542(" signal: rsact_ui::font::FontSize (clean)")

memo4294967588 ===o |sub|signal4294967542



signal4294967542 --> |clean|signal4294967542

signal4294967543 ===o |source|memo4294967588
signal4294967543(" signal: rsact_ui::layout::Layout (clean)")

memo4294967588 ===o |sub|signal4294967543




memo4294967545 ===o |source|memo4294967588
memo4294967545([" memo: alloc::string::String (clean)"])

memo4294967588 ===o |sub|memo4294967545



memo4294967545 --> |clean|memo4294967545

signal4294967547 ===o |source|memo4294967588
signal4294967547(" signal: rsact_ui::layout::Layout (clean)")

effect4294967581 ===o |sub|signal4294967547
effect4294967581[[" effect: () (dirty)"]]


signal4294967547 ===o |source|effect4294967581

signal4294967548 ===o |source|effect4294967581

signal4294967548(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (clean)")

memo4294967549 ===o |sub|signal4294967548
memo4294967549([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (clean)"])

memo4294967588 ===o |sub|memo4294967549


signal4294967548 ===o |source|memo4294967549


memo4294967549 --> |clean|memo4294967549

effect4294967581 ===o |sub|signal4294967548



signal4294967548 --> |clean|signal4294967548

signal4294967548 ===> |dirten|effect4294967581

memo4294967588 ===o |sub|signal4294967547




memo4294967549 ===o |source|memo4294967588

signal4294967550 ===o |source|memo4294967588

signal4294967552 ===o |source|memo4294967588

signal4294967554 ===o |source|memo4294967588
signal4294967554(" signal: rsact_ui::font::FontSize (clean)")

memo4294967588 ===o |sub|signal4294967554



signal4294967554 --> |clean|signal4294967554

signal4294967555 ===o |source|memo4294967588
signal4294967555(" signal: rsact_ui::layout::Layout (clean)")

memo4294967588 ===o |sub|signal4294967555




memo4294967557 ===o |source|memo4294967588
memo4294967557([" memo: alloc::string::String (clean)"])

memo4294967588 ===o |sub|memo4294967557



memo4294967557 --> |clean|memo4294967557

signal4294967559 ===o |source|memo4294967588
signal4294967559(" signal: rsact_ui::layout::Layout (clean)")

effect4294967582 ===o |sub|signal4294967559
effect4294967582[[" effect: () (dirty)"]]


signal4294967559 ===o |source|effect4294967582

signal4294967560 ===o |source|effect4294967582

signal4294967560(" signal: alloc::vec::Vec<rsact_ui::el::El<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>>> (clean)")

memo4294967561 ===o |sub|signal4294967560
memo4294967561([" memo: alloc::vec::Vec<rsact_reactive::memo::Memo<rsact_ui::layout::Layout>> (clean)"])

memo4294967588 ===o |sub|memo4294967561


signal4294967560 ===o |source|memo4294967561


memo4294967561 --> |clean|memo4294967561

effect4294967582 ===o |sub|signal4294967560



signal4294967560 --> |clean|signal4294967560

signal4294967560 ===> |dirten|effect4294967582

memo4294967588 ===o |sub|signal4294967559




memo4294967561 ===o |source|memo4294967588

signal4294967562 ===o |source|memo4294967588

signal4294967564 ===o |source|memo4294967588

memo4294967567 ===o |source|memo4294967588

signal4294967568 ===o |source|memo4294967588
signal4294967568(" signal: rsact_ui::layout::Layout (clean)")

memo4294967588 ===o |sub|signal4294967568



signal4294967568 --> |clean|signal4294967568

signal4294967570 ===o |source|memo4294967588

memo4294967573 ===o |source|memo4294967588

signal4294967575 ===o |source|memo4294967588
signal4294967575(" signal: rsact_ui::font::FontCtx (clean)")

memo4294967588 ===o |sub|signal4294967575





memo4294967588 --> |clean|memo4294967588

observer4294967642 ===o |sub|signal4294967530



signal4294967530 --> |clean|signal4294967530




memoChain4294967534(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))













memoChain4294967544(((" memoChain: rsact_ui::widget::icon::IconStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memoChain4294967546(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))








signal4294967551(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967553(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))






memoChain4294967556(((" memoChain: rsact_ui::widget::icon::IconStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))





memoChain4294967558(((" memoChain: rsact_ui::widget::text::TextStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))








signal4294967563(" signal: rsact_ui::widget::button::ButtonState (clean)")





memoChain4294967565(((" memoChain: rsact_ui::widget::button::ButtonStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (dirty)")))












signal4294967574(" signal: rsact_ui::page::dev::DevTools (clean)")






signal4294967577(" signal: rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")























signal4294967597(" 'Page state' signal: rsact_ui::widget::ctx::PageState<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>> (clean)")





memo4294967599([" memo: rsact_ui::widget::Meta (dirty)"])




memo4294967600([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (dirty)"])




memo4294967601([" 'Focusable' memo: alloc::vec::Vec<rsact_ui::el::ElId> (dirty)"])




memo4294967602([" 'Layout model' memo: rsact_ui::layout::LayoutModel (dirty)"])




signal4294967603(" 'Page style' signal: rsact_ui::page::PageStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")




signal4294967604(" 'Force redraw' signal: () (clean)")




signal4294967605(" 'Page state' signal: rsact_ui::widget::ctx::PageState<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>> (clean)")










memo4294967612([" memo: rsact_ui::widget::Meta (dirty)"])




memo4294967613([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (dirty)"])




memo4294967614([" 'Focusable' memo: alloc::vec::Vec<rsact_ui::el::ElId> (dirty)"])




memo4294967615([" 'Layout model' memo: rsact_ui::layout::LayoutModel (dirty)"])




signal4294967616(" 'Page style' signal: rsact_ui::page::PageStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")




signal4294967617(" 'Force redraw' signal: () (clean)")




signal4294967618(" 'Page state' signal: rsact_ui::widget::ctx::PageState<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>> (clean)")









memo4294967624([" memo: rsact_ui::widget::Meta (dirty)"])




memo4294967625([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (dirty)"])




memo4294967626([" 'Focusable' memo: alloc::vec::Vec<rsact_ui::el::ElId> (dirty)"])




memo4294967627([" 'Layout model' memo: rsact_ui::layout::LayoutModel (dirty)"])




signal4294967628(" 'Page style' signal: rsact_ui::page::PageStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")




signal4294967629(" 'Force redraw' signal: () (clean)")




signal4294967630(" 'Page state' signal: rsact_ui::widget::ctx::PageState<rsact_ui::widget::ctx::Wtf<rsact_ui::render::buffer::BufferRenderer<embedded_graphics_core::pixelcolor::binary_color::BinaryColor>, rsact_ui::style::NullStyler, &str>> (clean)")






memo4294967633([" memo: rsact_ui::widget::Meta (dirty)"])




memo4294967634([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (dirty)"])




memo4294967635([" memo: rsact_ui::widget::Meta (dirty)"])




memo4294967636([" memo: alloc::vec::Vec<rsact_reactive::memo::MemoTree<rsact_ui::widget::Meta>> (dirty)"])




memo4294967637([" 'Focusable' memo: alloc::vec::Vec<rsact_ui::el::ElId> (dirty)"])




memo4294967638([" 'Layout model' memo: rsact_ui::layout::LayoutModel (dirty)"])




signal4294967639(" 'Page style' signal: rsact_ui::page::PageStyle<embedded_graphics_core::pixelcolor::binary_color::BinaryColor> (clean)")




signal4294967640(" 'Force redraw' signal: () (clean)")






```