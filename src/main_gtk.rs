use euclid::default::Size2D;
use gio::prelude::*;
use gtk::prelude::*;
use gleam::gl::{self, Gl};
use simpleservo::*;
use std::cell::{Cell, RefCell};
use std::env::args;
use std::rc::Rc;

struct Waker(());
impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Waker(self.0.clone()))
    }
    fn wake(&self) {
        //let _ = self.0.send_event(());
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

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("First GTK+ Program");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(1024, 768);

    //let button = gtk::Button::with_label("Click me!");

    //window.add(&button);

    let glarea = gtk::GLArea::new();
    let gl = Rc::new(RefCell::new(None));
    let gl2 = gl.clone();
    let gtk_context = Rc::new(RefCell::new(None));
    let gtk_context2 = gtk_context.clone();
    let gtk_context3 = gtk_context.clone();
    let fb = Rc::new(RefCell::new(0));
    let fb2 = fb.clone();

    glarea.connect_realize(move |widget| {
        if widget.get_realized() {
	    widget.make_current();
        }

        let context = unsafe { cgl::CGLGetCurrentContext() };
        let context = surfman::NativeContext(context);

        let allocation = widget.get_allocation();

        let gl = gl_glue::gl::init().unwrap();
        *gl2.borrow_mut() = Some(gl.clone());

        *fb.borrow_mut() = gl.gen_framebuffers(1)[0];

        let mut prefs = std::collections::HashMap::new();
        if let Some(arg) = std::env::args().nth(1) {
            prefs.insert("shell.homepage".to_owned(), PrefValue::Str(arg));
        }

        let opts = InitOptions {
            args: vec![],
            coordinates: Coordinates::new(
                0, 0,
                allocation.width, allocation.height,
                allocation.width, allocation.height,
            ),
            density: 2.0, //FIXME
            prefs: Some(prefs),
            xr_discovery: None,
            surfman_integration: SurfmanIntegration::Surface,

            // only used for media hardware acceleration
            gl_context_pointer: None, 
            native_display_pointer: None,
        };

        init(opts, gl, Box::new(Waker(())), Box::new(Callbacks)).unwrap();

        call(|s| unsafe {
            let context = s.surfman().device().create_context_from_native_context(context).unwrap();
            *gtk_context.borrow_mut() = Some(context);
            Ok(())
        });
    });

    /*glarea.connect_resize(|_widget, width, height| {
        call(|s| {
            s.resize(Coordinates::new(
                0, 0,
                width, height,
                width, height,
            ))
        });
    });*/

    let gl2 = gl.clone();
    glarea.connect_render(move |widget, _gl_context| {
        let gl2 = gl2.borrow();
        let gl = gl2.as_ref().unwrap();
        let _ = gl.get_error();

        let allocation = widget.get_allocation();
        let mut wrapped_context = gtk_context2.borrow_mut();
        let wrapped_context = wrapped_context.as_mut().unwrap();
        assert_eq!(gl.get_error(), gl::NO_ERROR);

        call(|s| {
            s.surfman().with_front_buffer(|device, surface| {
                assert_eq!(gl.get_error(), gl::NO_ERROR);
                let info = device.surface_info(&surface);
                let texture = device.create_surface_texture(wrapped_context, surface).unwrap();
                assert_eq!(gl.get_error(), gl::NO_ERROR);
                let texture_id = device.surface_texture_object(&texture);
                draw_texture(
                    &**gl,
                    device,
                    *fb2.borrow(),
                    texture_id,
                    info.size,
                    Size2D::new(allocation.width, allocation.height)
                );
                let surface = device.destroy_surface_texture(wrapped_context, texture).unwrap();
                surface
            });
            Ok(())
        });

        Inhibit(false)
    });

    let shutting_down = Rc::new(Cell::new(false));
    let shutting_down2 = shutting_down.clone();
    application.connect_shutdown(move |_| {
        let mut wrapped_context = gtk_context3.borrow_mut();
        call(|s| {
            s.surfman().device().destroy_context(&mut wrapped_context.take().unwrap()).unwrap();
            shutting_down.set(true);
            s.request_shutdown()
        });
    });

    glib::idle_add_local(move || {
        if !shutting_down2.get() {
            call(|s| s.perform_updates());
        }
        Continue(!shutting_down2.get())
    });

    window.add(&glarea);

    window.show_all();
}

fn draw_texture(
    gl: &dyn Gl,
    device: &surfman::Device,
    texture: u32,
    fb: u32,
    size: Size2D<i32>,
    dest: Size2D<i32>,
) {
    gl.bind_framebuffer(gl::READ_FRAMEBUFFER, fb);
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    gl.framebuffer_texture_2d(
        gl::READ_FRAMEBUFFER,
        gl::COLOR_ATTACHMENT0,
        device.surface_gl_texture_target(),
        texture,
        0,
    );
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    gl.blit_framebuffer(
        0, 0, size.width, size.height,
        0, 0, dest.width, dest.height,
        gl::COLOR_BUFFER_BIT,
        gl::LINEAR,
    );
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    gl.bind_framebuffer(gl::READ_FRAMEBUFFER, 0);
    assert_eq!(gl.get_error(), gl::NO_ERROR);
    gl.flush();
}

fn main() {
    env_logger::init();

    let application =
        gtk::Application::new(Some("com.github.gtk-rs.examples.basic"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&args().collect::<Vec<_>>());
}
