// FIXME:

fn main() {}
/*
use femtovg::FontId;
use glutin::event::VirtualKeyCode;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

use rand::Rng;

use femtovg::{
    //CompositeOperation,
    renderer::OpenGl,
    Align,
    Baseline,
    Canvas,
    Color,
    Paint,
    Path,
};

use std::collections::HashMap;

use morphorm::{layout, Cache, LayoutType, Node, Units, Units::*};

#[derive(Default, Clone)]
pub struct Widget {
    child: Vec<Widget>,
    main: Units,
    cross: Units,
    main_before: Units,
    main_after: Units,
    cross_before: Units,
    cross_after: Units,
    layout_type: LayoutType,
    color: femtovg::Color,
    id: u32,
}

impl Widget {
    pub fn new(id: u32, main: Units, cross: Units) -> Self {
        let random_red: u8 = rand::thread_rng().gen();
        let random_green: u8 = rand::thread_rng().gen();
        let random_blue: u8 = rand::thread_rng().gen();

        Self {
            id,
            color: femtovg::Color::rgb(random_red, random_green, random_blue),
            main,
            cross,
            main_before: Units::Auto,
            main_after: Units::Auto,
            cross_before: Units::Auto,
            cross_after: Units::Auto,
            ..Default::default()
        }
    }
}

impl<'t> Node<'t> for &'t Widget {
    type Store = ();
    type Tree = ();
    type ChildIter = std::slice::Iter<'t, Widget>;
    type CacheKey = u32;

    fn children(&self, tree: &'t Self::Tree) -> Self::ChildIter {
        self.child.iter()
    }

    fn key(&self) -> Self::CacheKey {
        self.id
    }

    fn main(&self, store: &Self::Store) -> Option<Units> {
        Some(self.main)
    }

    fn height(&self, store: &Self::Store) -> Option<Units> {
        Some(self.height)
    }

    fn layout_type(&self, store: &Self::Store) -> Option<morphorm::LayoutType> {
        Some(self.layout_type)
    }
}

#[derive(Default)]
pub struct LayoutCache {
    rect: HashMap<u32, (f32, f32, f32, f32)>,
}

impl Cache for LayoutCache {
    type Node = u32;

    fn width(&self, node: Self::Node) -> f32 {
        if let Some(rect) = self.rect.get(&node) {
            return rect.0;
        }

        0.0
    }

    fn height(&self, node: Self::Node) -> f32 {
        if let Some(rect) = self.rect.get(&node) {
            return rect.1;
        }

        0.0
    }

    fn posx(&self, node: Self::Node) -> f32 {
        if let Some(rect) = self.rect.get(&node) {
            return rect.2;
        }

        0.0
    }

    fn posy(&self, node: Self::Node) -> f32 {
        if let Some(rect) = self.rect.get(&node) {
            return rect.3;
        }

        0.0
    }

    fn set_width(&mut self, node: Self::Node, width: f32) {
        if let Some(rect) = self.rect.get_mut(&node) {
            rect.0 = width;
        } else {
            self.rect.insert(node, (width, 0.0, 0.0, 0.0));
        }
    }

    fn set_height(&mut self, node: Self::Node, height: f32) {
        if let Some(rect) = self.rect.get_mut(&node) {
            rect.1 = height;
        } else {
            self.rect.insert(node, (0.0, height, 0.0, 0.0));
        }
    }

    fn set_posx(&mut self, node: Self::Node, posx: f32) {
        if let Some(rect) = self.rect.get_mut(&node) {
            rect.2 = posx;
        } else {
            self.rect.insert(node, (0.0, 0.0, posx, 0.0));
        }
    }

    fn set_posy(&mut self, node: Self::Node, posy: f32) {
        if let Some(rect) = self.rect.get_mut(&node) {
            rect.3 = posy;
        } else {
            self.rect.insert(node, (0.0, 0.0, 0.0, posy));
        }
    }
}

fn main() {
    let mut cache = LayoutCache::default();
    let mut root = Widget::new(0, Pixels(100.0), Pixels(100.0));
    root.child.push(Widget::new(1, Pixels(40.0), Pixels(40.0)));
    layout(&&root, &mut cache, &(), &());
    render(cache, root);
}

pub fn render(mut cache: LayoutCache, root: Widget) {
    let el = EventLoop::new();

    let (renderer, windowed_context) = {
        use glutin::ContextBuilder;

        let wb = WindowBuilder::new()
            .with_inner_size(winit::dpi::PhysicalSize::new(1000i32, 600i32))
            .with_title("Morphorm Demo");

        let windowed_context =
            ContextBuilder::new().with_vsync(false).build_windowed(wb, &el).unwrap();
        let windowed_context = unsafe { windowed_context.make_current().unwrap() };

        let renderer = OpenGl::new(|s| windowed_context.get_proc_address(s) as *const _)
            .expect("Cannot create renderer");

        (renderer, windowed_context)
    };

    let mut canvas = Canvas::new(renderer).expect("Cannot create canvas");

    let font =
        canvas.add_font("examples/common/Roboto-Regular.ttf").expect("Failed to load font file");

    //world.cache.set_width(root, 1000.0);
    //world.cache.set_height(root, 600.0);

    el.run(move |event, _, control_flow| {
        #[cfg(not(target_arch = "wasm32"))]
        let window = windowed_context.window();

        *control_flow = ControlFlow::Poll;

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { ref event, .. } => match event {
                WindowEvent::Resized(physical_size) => {
                    windowed_context.resize(*physical_size);
                    //world.set_width(root, Units::Pixels(physical_size.width as f32));
                    //world.set_height(root, Units::Pixels(physical_size.height as f32));
                    //world.cache.set_width(root, physical_size.width as f32);
                    //world.cache.set_height(root, physical_size.height as f32);

                    layout(&&root, &mut cache, &(), &());
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,

                WindowEvent::KeyboardInput { device_id: _, input, is_synthetic: _ } => {
                    if input.virtual_keycode == Some(VirtualKeyCode::H) {
                        //let nodes = world.tree.flatten();
                        // for node in nodes.into_iter() {
                        //     println!(
                        //         "{:?} px: {:?} py: {:?} w: {:?} h: {:?}",
                        //         node,
                        //         world.cache.posx(node),
                        //         world.cache.posy(node),
                        //         world.cache.width(node),
                        //         world.cache.height(node)
                        //     );
                        // }
                    }
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                let dpi_factor = window.scale_factor();
                let size = window.inner_size();

                canvas.set_size(size.width as u32, size.height as u32, dpi_factor as f32);
                canvas.clear_rect(
                    0,
                    0,
                    size.width as u32,
                    size.height as u32,
                    Color::rgbf(0.3, 0.3, 0.32),
                );

                draw_node(&root, &cache, &mut canvas, font);

                canvas.flush();
                windowed_context.swap_buffers().unwrap();
            }
            Event::MainEventsCleared => window.request_redraw(),
            _ => (),
        }
    });
}

fn draw_node(node: &Widget, cache: &LayoutCache, canvas: &mut Canvas<OpenGl>, font: FontId) {
    let posx = cache.posx(node.key());
    let posy = cache.posy(node.key());
    let width = cache.width(node.key());
    let height = cache.height(node.key());

    println!("{} {} {} {} {}", node.id, width, height, posx, posy);

    let color = node.color;

    let mut path = Path::new();
    path.rect(posx, posy, width, height);
    let paint = Paint::color(color);
    canvas.fill_path(&mut path, paint);

    let mut paint = Paint::color(Color::black());
    paint.set_font_size(24.0);
    paint.set_text_align(Align::Center);
    paint.set_text_baseline(Baseline::Middle);
    paint.set_font(&vec![font]);
    let _ =
        canvas.fill_text(posx + width / 2.0, posy + height / 2.0, &node.key().to_string(), paint);

    for child in (&node).children(&()) {
        draw_node(child, cache, canvas, font);
    }
}
*/
