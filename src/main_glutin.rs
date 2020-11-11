use euclid::Size2D;
use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use glutin::window::WindowBuilder;
use glutin::ContextBuilder;
use glutin::platform::ContextTraitExt;
use simpleservo::*;
use std::cell::RefCell;

mod support;

fn main() {
    env_logger::init();
    let el = EventLoop::new();
    let proxy = el.create_proxy();
    let wb = WindowBuilder::new().with_title("A fantastic window!");

    let windowed_context =
        ContextBuilder::new().build_windowed(wb, &el).unwrap();

    let windowed_context = unsafe { windowed_context.make_current().unwrap() };
    let window = windowed_context.window();
    let size = window.inner_size().cast::<i32>();

    println!(
        "Pixel format of the window's GL context: {:?}",
        windowed_context.get_pixel_format()
    );

    let gl = support::load(&windowed_context.context());

    let mut prefs = std::collections::HashMap::new();
    if let Some(arg) = std::env::args().nth(1) {
        prefs.insert("shell.homepage".to_owned(), PrefValue::Str(arg));
    }

    let gl2 = gl_glue::gl::init().unwrap();
    let opts = InitOptions {
        args: vec![],
        coordinates: Coordinates::new(
            0, 0,
            size.width, size.height,
            size.width, size.height,
        ),
        density: window.scale_factor() as f32,
        prefs: Some(prefs),
        xr_discovery: None,
        surfman_integration: SurfmanIntegration::Surface,

        // only used for media hardware acceleration
        gl_context_pointer: None, 
        native_display_pointer: None,
    };
    struct Waker(EventLoopProxy<()>);
    impl EventLoopWaker for Waker {
        fn clone_box(&self) -> Box<dyn EventLoopWaker> {
            Box::new(Waker(self.0.clone()))
        }
        fn wake(&self) {
            let _ = self.0.send_event(());
        }
    }
    struct Callbacks;
    impl HostTrait for Callbacks {
        fn prompt_alert(&self, _msg: String, _trusted: bool) {}
        fn prompt_yes_no(&self, _msg: String, _trusted: bool) -> PromptResult { PromptResult::Primary }
        fn prompt_ok_cancel(&self, _msg: String, _trusted: bool) -> PromptResult { PromptResult::Primary }
        fn prompt_input(&self, _msg: String, _default: String, _trusted: bool) -> Option<String> { None }
        fn show_context_menu(&self, _title: Option<String>, _items: Vec<String>) {}
        fn on_load_started(&self) {}
        fn on_load_ended(&self) {}
        fn on_title_changed(&self, _title: Option<String>) {}
        fn on_allow_navigation(&self, _url: String) -> bool { true }
        fn on_url_changed(&self, _url: String) {}
        fn on_history_changed(&self, _can_go_back: bool, _can_go_forward: bool) {}
        /// Page animation state has changed. If animating, it's recommended
        /// that the embedder doesn't wait for the wake function to be called
        /// to call perform_updates. Usually, it means doing:
        /// while true { servo.perform_updates() }. This will end up calling flush
        /// which will call swap_buffer which will be blocking long enough to limit
        /// drawing at 60 FPS.
        /// If not animating, call perform_updates only when needed (when the embedder
        /// has events for Servo, or Servo has woken up the embedder event loop via
        /// EventLoopWaker).
        fn on_animating_changed(&self, _animating: bool) {}
        fn on_shutdown_complete(&self) {
            deinit();
        }
        fn on_ime_show(&self, _input_type: InputMethodType, _text: Option<String>, _bounds: DeviceIntRect) {}
        fn on_ime_hide(&self) {}
        fn get_clipboard_contents(&self) -> Option<String> { None }
        fn set_clipboard_contents(&self, _contents: String) {}
        fn on_media_session_metadata(&self, _title: String, _artist: String, _album: String) {}
        fn on_media_session_playback_state_change(&self, _state: MediaSessionPlaybackState) {}
        fn on_media_session_set_position_state(&self, _duration: f64, _position: f64, _playback_rate: f64) {}
        fn on_devtools_started(&self, _port: Result<u16, ()>, _token: String) {}
        fn on_panic(&self, _reason: String, _backtrace: Option<String>) {}
    }
    init(opts, gl2, Box::new(Waker(proxy)), Box::new(Callbacks)).unwrap();

    let mut wrapped_context = call(|s| {
        Ok(unsafe {
            s.surfman_device().create_context_from_native_context(
                surfman::NativeContext(windowed_context.context().raw_handle()),
            ).unwrap()
        })
    });

    let windowed_context = RefCell::new(Some(windowed_context));

    el.run(move |event, _, control_flow| {
        //println!("{:?}", event);
        *control_flow = ControlFlow::Wait;

        match event {
            Event::LoopDestroyed => {
                call(|s| s.request_shutdown());
                return;
            },
            Event::WindowEvent { event, .. } => {
                call(|s| s.perform_updates());
                match event {
                    WindowEvent::Resized(physical_size) => {
                        let windowed_context2 = windowed_context.borrow_mut().take().unwrap();
                        windowed_context2.resize(physical_size);
                        *windowed_context.borrow_mut() = Some(windowed_context2);
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit
                    }
                    _ => (),
                }
            },
            Event::RedrawRequested(_) => {
                let windowed_context2 = windowed_context.borrow_mut().take().unwrap();
                let windowed_context2 = unsafe { windowed_context2.make_current() }.unwrap();
                call(|s| {
                    s.share(|device, surface| {
                        let info = device.surface_info(&surface);
                        let texture = device.create_surface_texture(&mut wrapped_context, surface).unwrap();
                        let texture_id = device.surface_texture_object(&texture);
                        gl.draw_texture(device, texture_id, info.size, Size2D::new(size.width, size.height));
                        let surface = device.destroy_surface_texture(&mut wrapped_context, texture).unwrap();
                        surface
                    });
                    Ok(())
                });
                windowed_context2.swap_buffers().unwrap();
                *windowed_context.borrow_mut() = Some(windowed_context2);
                
            }
            Event::UserEvent(()) => {
                call(|s| s.perform_updates());
                windowed_context.borrow().as_ref().unwrap().window().request_redraw();
            }
            _ => (),
        }
    });
}

fn call<T, F>(f: F) -> T
where
    F: FnOnce(&mut ServoGlue) -> Result<T, &'static str>,
{
    match SERVO.with(|s| match s.borrow_mut().as_mut() {
        Some(ref mut s) => (f)(s),
        None => Err("Servo not available in this thread"),
    }) {
        Err(e) => panic!(e),
        Ok(r) => r,
    }
}
