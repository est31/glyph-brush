extern crate gfx;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate time;
extern crate pretty_env_logger;
extern crate gfx_glyph;

use glutin::GlContext;
use gfx::{format, Device};
use std::env;

fn main() {
    pretty_env_logger::init().expect("log");

    // winit wayland is currently still wip
    if cfg!(target_os = "linux") && env::var("WINIT_UNIX_BACKEND").is_err() {
        env::set_var("WINIT_UNIX_BACKEND", "x11");
    }

    if cfg!(debug_assertions) && env::var("yes_i_really_want_debug_mode").is_err() {
        eprintln!("Note: Release mode will improve performance greatly.\n    \
            e.g. use `cargo run --example paragraph --release`");
    }

    let mut events_loop = glutin::EventsLoop::new();
    let window_builder = glutin::WindowBuilder::new()
        .with_title("gfx_glyph example - scroll to zoom, type to modify".to_string())
        .with_dimensions(1024, 576);
    let context = glutin::ContextBuilder::new();
    let (window, mut device, mut factory, mut main_color, mut main_depth) =
        gfx_window_glutin::init::<format::Srgba8, format::Depth>(window_builder, context, &events_loop);

    let mut glyph_brush = gfx_glyph::GlyphBrushBuilder::using_font(include_bytes!("Arial Unicode.ttf"))
        // .initial_cache_size((1024, 1024))
        .gpu_cache_position_tolerance(0.2)
        // lower position tolerance seems to cause missing-character render issues in rusttype
        // currently. So disabling it cause every-frame re-draws which seem to be less flawed...
        .cache_glyph_drawing(false)
        .build(factory.clone());

    let mut text: String = include_str!("lipsum.txt").into();

    let mut encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();

    let mut running = true;
    let mut font_size = gfx_glyph::Scale::uniform(18.0 * window.hidpi_factor());
    while running {
        events_loop.poll_events(|event| {
            use glutin::*;

            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::Closed => running = false,
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(keypress),
                            .. },
                        ..
                    } => match keypress {
                        VirtualKeyCode::Escape => running = false,
                        VirtualKeyCode::Back => { text.pop(); },
                        _ => (),
                    },
                    WindowEvent::ReceivedCharacter(c) => if c != '\u{7f}' && c != '\u{8}' {
                        text.push(c);
                    },
                    WindowEvent::Resized(width, height) => {
                        window.resize(width, height);
                        gfx_window_glutin::update_views(&window, &mut main_color, &mut main_depth);
                    },
                    WindowEvent::MouseWheel{ delta: MouseScrollDelta::LineDelta(_, y), .. } => {
                        // increase/decrease font size with mouse wheel
                        let mut size = font_size.x / window.hidpi_factor();
                        if y < 0.0 { size += (size / 4.0).max(2.0) }
                        else { size *= 4.0 / 5.0 };
                        size = size.max(1.0);
                        font_size = gfx_glyph::Scale::uniform(size * window.hidpi_factor());
                    },
                    _ => {},
                }
            }
        });

        encoder.clear(&main_color, [0.02, 0.02, 0.02, 1.0]);

        let (width, height, ..) = main_color.get_dimensions();

        // The section is all the info needed for the glyph brush to render a 'section' of text
        // can use `..Section::default()` to skip the bits you don't care about
        // also see convenience variants StaticSection & OwnedSection
        let section = gfx_glyph::Section {
            text: &text,
            scale: font_size,
            screen_position: (0.0, 0.0),
            bounds: (width as f32 / 3.15, height as f32),
            color: [0.9, 0.3, 0.3, 1.0],
        };

        // the lib needs layout logic to render the glyphs, ie a gfx_glyph::GlyphPositioner
        // See the built-in ones at Layout::*
        // Layout::default() is a left aligned word wrapping style
        let layout = gfx_glyph::Layout::default();

        // Adds a section & layout to the queue for the next call to `draw_queued`, this
        // can be called multiple times for different sections that want to use the same
        // font and gpu cache
        // This step computes the glyph positions, this is cached to avoid unnecessary recalculation
        glyph_brush.queue(section, &layout);

        use gfx_glyph::*;
        glyph_brush.queue(Section {
            text: &text,
            scale: font_size,
            screen_position: (width as f32 / 2.0, 0.0),
            bounds: (width as f32 / 3.15, height as f32),
            color: [0.3, 0.9, 0.3, 1.0],
        }, &Layout::Wrap(GlyphGroup::Word, HorizontalAlign::Center));

        glyph_brush.queue(Section {
            text: &text,
            scale: font_size,
            screen_position: (width as f32, 0.0),
            bounds: (width as f32 / 3.15, height as f32),
            color: [0.3, 0.3, 0.9, 1.0],
        }, &Layout::Wrap(GlyphGroup::Word, HorizontalAlign::Right));

        // Finally once per frame you want to actually draw all the sections you've submitted
        // with `queue` calls.
        //
        // Note: Drawing in the case the text is unchanged from the previous frame (a common case)
        // is essentially free as the vertices are reused &  gpu cache updating interaction
        // can be skipped.
        glyph_brush.draw_queued(&mut encoder, &main_color).expect("draw");

        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}