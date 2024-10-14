# rsact-encoder

rsact widget library targeting projects with encoder and button.


## Tutorial and best practices

### Dialogs & popups

For now, overlays are not implemented in rsact-ui so you cannot create something like a dialog window, and I recommend to use separate pages as dialogs.
But for small displays it is much more logical to use pages, as there's not enough space to fit dialog as an overlay distinctly.
Example of page as a dialog window: TODO
<!-- ```rs
fn dialog<W: WidgetCtx>(prev_page: PageId) -> impl 
``` -->
