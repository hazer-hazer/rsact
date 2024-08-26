use embedded_graphics::{
    pixelcolor::Rgb888,
    prelude::{Dimensions, RgbColor},
};
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, Window,
};
use rand::{random, Rng};
use rsact_ui::{
    axis::{Axial, Axis},
    block::{Block, Border, BoxModel},
    el::El,
    event::{EventStub, Propagate},
    layout::{Align, Layout, LayoutTree, Limits},
    render::{color::Color, Renderer},
    size::{Length, Size},
    ui::UI,
    widget::{DrawCtx, LayoutCtx, Widget, WidgetCtx},
};
use std::array;

struct FlexBox<C: WidgetCtx> {
    size: Size<Length>,
    wrap: bool,
    axis: Axis,
    gap: Size,
    horizontal_align: Align,
    vertical_align: Align,
    children: Vec<El<C>>,
    color: C::Color,
}

impl<C: WidgetCtx> Widget<C> for FlexBox<C> {
    fn children(&self) -> &[El<C>] {
        &self.children
    }

    fn size(&self) -> Size<Length> {
        self.size
    }

    fn content_size(&self) -> Limits {
        let children_limits = self
            .children
            .iter()
            .map(|child| child.content_size())
            .collect::<Vec<_>>();
        let min_content = children_limits
            .iter()
            .fold(Size::zero(), |min, child| min.min(child.min()));
        let max_content =
            children_limits.iter().fold(Size::zero(), |max, child| {
                self.axis.canon(
                    max.main(self.axis) + child.max().main(self.axis),
                    max.cross(self.axis).max(child.max().cross(self.axis)),
                )
            });
        Limits::new(min_content, max_content)
    }

    fn layout(&self, _ctx: &LayoutCtx<'_, C>) -> Layout {
        Layout::Flex {
            wrap: self.wrap,
            axis: self.axis,
            gap: self.gap,
            box_model: BoxModel::new().padding(0).border(0),
            horizontal_align: self.horizontal_align,
            vertical_align: self.vertical_align,
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> rsact_ui::widget::DrawResult {
        ctx.renderer.block(Block {
            border: Border::zero(),
            rect: ctx.layout.area,
            background: Some(self.color),
        })?;

        ctx.draw_children(&self.children)
    }
}

struct Item<C: WidgetCtx> {
    size: Size<Length>,
    color: <C::Renderer as Renderer>::Color,
}

impl<C: WidgetCtx> Widget<C> for Item<C> {
    fn size(&self) -> Size<Length> {
        self.size
    }

    fn content_size(&self) -> Limits {
        Limits::unknown()
    }

    fn layout(&self, _ctx: &LayoutCtx<'_, C>) -> Layout {
        Layout::Edge
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, C>) -> rsact_ui::widget::DrawResult {
        ctx.renderer.block(Block::new_filled(ctx.layout.area, Some(self.color)))
    }
}

fn main() {
    let mut rng = rand::thread_rng();

    // let items: [_; 50] = array::from_fn(|_| {
    //     (rng.gen_range(10..=50).into(), rng.gen_range(10..=50).into())
    // });

    let items = [(Length::Fixed(50), 50.into()); 5];
    // let items = [(Length::Div(5), 50.into()), (Length::Div(6), 50.into())];
    // let items = [
    //     (Length::Div(5), 50.into()),
    //     (Length::Div(4), 50.into()),
    //     (Length::Div(3), 50.into()),
    //     (Length::Div(2), 50.into()),
    //     (Length::Div(2), 50.into()),
    //     (Length::Div(1), 50.into()),
    // ];

    let flexbox = FlexBox {
        wrap: true,
        size: Size::new(250.into(), 250.into()),
        axis: Axis::X,
        gap: Size::new(5, 5),
        horizontal_align: Align::Start,
        vertical_align: Align::End,
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
