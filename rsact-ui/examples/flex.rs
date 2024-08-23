use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::random;
use rsact_ui::{
    axis::Axis,
    block::{Block, Border, BoxModel},
    el::El,
    event::{EventStub, Propagate},
    layout::{Align, Layout, LayoutKind, LayoutTree},
    render::{color::Color, Renderer},
    size::{Length, Size},
    ui::UI,
    widget::{Widget, WidgetCtx},
};

struct FlexBox<C: WidgetCtx> {
    size: Size<Length>,
    axis: Axis,
    gap: u32,
    horizontal_align: Align,
    vertical_align: Align,
    children: Vec<El<C>>,
    color: <C::Renderer as Renderer>::Color,
}

impl<C: WidgetCtx> Widget<C> for FlexBox<C> {
    fn children(&self) -> &[El<C>] {
        &self.children
    }

    fn size(&self) -> Size<Length> {
        self.size
    }

    fn layout(&self, ctx: &rsact_ui::widget::Ctx<C>) -> Layout {
        LayoutKind::Flex {
            axis: self.axis,
            gap: self.gap,
            box_model: BoxModel::new().padding(0).border(0),
            horizontal_align: self.horizontal_align,
            vertical_align: self.vertical_align,
        }
        .into_layout(self.size)
    }

    fn draw(
        &self,
        ctx: &rsact_ui::widget::Ctx<C>,
        renderer: &mut <C as WidgetCtx>::Renderer,
        layout: &LayoutTree,
    ) -> rsact_ui::widget::DrawResult {
        renderer.block(Block {
            border: Border::zero(),
            rect: layout.area,
            background: Some(self.color),
        })?;

        self.children
            .iter()
            .zip(layout.children())
            .try_for_each(|child| child.0.draw(ctx, renderer, &child.1))
    }

    fn on_event(
        &mut self,
        ctx: &mut rsact_ui::widget::Ctx<C>,
        event: <C as WidgetCtx>::Event,
    ) -> rsact_ui::event::EventResponse<<C as WidgetCtx>::Event> {
        Propagate::Ignored.into()
    }
}

struct Item<C: WidgetCtx> {
    size: Size<Length>,
    color: <C::Renderer as Renderer>::Color,
}

impl<C: WidgetCtx> Widget<C> for Item<C> {
    fn children(&self) -> &[rsact_ui::el::El<C>] {
        &[]
    }

    fn size(&self) -> Size<Length> {
        self.size
    }

    fn layout(
        &self,
        ctx: &rsact_ui::widget::Ctx<C>,
    ) -> rsact_ui::layout::Layout {
        LayoutKind::Edge.into_layout(self.size)
    }

    fn draw(
        &self,
        ctx: &rsact_ui::widget::Ctx<C>,
        renderer: &mut C::Renderer,
        layout: &LayoutTree,
    ) -> rsact_ui::widget::DrawResult {
        renderer.block(Block::new_filled(layout.area, Some(self.color)))
    }

    fn on_event(
        &mut self,
        ctx: &mut rsact_ui::widget::Ctx<C>,
        event: C::Event,
    ) -> rsact_ui::event::EventResponse<C::Event> {
        Propagate::Ignored.into()
    }
}

fn main() {
    let items = [(50.into(), 50.into()); 15];

    let flexbox = FlexBox {
        size: Size::new(250.into(), 250.into()),
        axis: Axis::X,
        gap: 0,
        horizontal_align: Align::Start,
        vertical_align: Align::Center,
        children: items
            .into_iter()
            .map(|size| {
                Item {
                    size: Size::from(size),
                    color: Rgb888::new(random(), random(), random()),
                }
                .el()
            })
            .collect(),
        color: Rgb888::WHITE,
    };

    let output_settings = OutputSettingsBuilder::new().scale(2).build();

    let mut window = Window::new("TEST", &output_settings);

    let mut display =
        SimulatorDisplay::<Rgb888>::new(Size::new(480, 270).into());

    window.update(&display);

    let mut ui = UI::new(flexbox.el(), display.bounding_box().size);

    loop {
        window.events().for_each(|e| {});
        ui.tick([EventStub].into_iter());
        ui.draw(&mut display).unwrap();

        window.update(&display);
    }
}
