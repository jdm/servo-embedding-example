use euclid::{Point2D, Size2D, Scale};
use glutin::dpi::{PhysicalPosition, PhysicalSize};
use glutin::event::{Event, WindowEvent, MouseButton, ElementState};
use glutin::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use glutin::window::{CursorIcon, WindowBuilder};
use glutin::ContextBuilder;
//use glutin::WindowedContext;
use glutin::platform::ContextTraitExt;
use raw_window_handle::{/*HasRawWindowHandle,*/ HasRawDisplayHandle};
use servo::*;
//use servo::config::prefs::PrefValue;
use servo::base::id::WebViewId;
use servo::compositing::CompositeTarget;
use servo::compositing::windowing::EmbedderCoordinates;
use servo::compositing::windowing::AnimationState;
use servo::compositing::windowing::{EmbedderMethods, EmbedderEvent, WindowMethods, MouseWindowEvent};
use servo::config::opts::Opts;
use servo::config::prefs::Preferences;
use servo::embedder_traits::{EventLoopWaker, EmbedderMsg, CompositorEventVariant, Cursor};
use servo::script_traits::MouseButton as ServoMouseButton;
use servo::webrender_api::units::{DeviceIntRect, DeviceIntPoint};
use servo::webrender_traits::RenderingContext;
use servo::webrender_api::units::DevicePixel;
use servo::url::ServoUrl;
use surfman::Connection;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod support;

fn glutin_size_to_euclid_size<T>(size: PhysicalSize<T>) -> Size2D<T, DevicePixel> {
    Size2D::new(size.width, size.height)
}

fn glutin_position_to_euclid_point<T>(position: PhysicalPosition<T>) -> Point2D<T, DevicePixel> {
    Point2D::new(position.x, position.y)
}

fn main() {
    //env_logger::init();
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Error initializing crypto provider");

    let el = EventLoop::new();
    let proxy = el.create_proxy();
    let wb = WindowBuilder::new().with_title("A fantastic window!");

    let windowed_context =
        ContextBuilder::new().build_windowed(wb, &el).unwrap();

    let windowed_context = unsafe { windowed_context.make_current().unwrap() };
    let window = windowed_context.window();
    //let size = window.inner_size().cast::<i32>();

    println!(
        "Pixel format of the window's GL context: {:?}",
        windowed_context.get_pixel_format()
    );

    let gl = support::load(&windowed_context.context());

    struct Waker(EventLoopProxy<()>);
    impl EventLoopWaker for Waker {
        fn clone_box(&self) -> Box<dyn EventLoopWaker> {
            Box::new(Waker(self.0.clone()))
        }
        fn wake(&self) {
            let _ = self.0.send_event(());
        }
    }
    struct Embedder {
        waker: Waker,
    }
    impl EmbedderMethods for Embedder {
        fn create_event_loop_waker(&mut self) -> Box<dyn EventLoopWaker> {
            self.waker.clone_box()
        }
    }

    struct Window {
        coordinates: RefCell<EmbedderCoordinates>,
        animating: Cell<bool>,
    }
    impl WindowMethods for Window {
        fn get_coordinates(&self) -> EmbedderCoordinates {
            self.coordinates.borrow().clone()
        }
        fn set_animation_state(&self, state: AnimationState) {
            self.animating.set(state == AnimationState::Animating);
            //println!("animation state: {:?}", _state);
        }
        /*fn rendering_context(&self) -> RenderingContext {
            self.rendering_context.clone()
        }*/
    }

    // Initialize surfman
    let display_handle = window
        .raw_display_handle();
    let connection =
        Connection::from_raw_display_handle(display_handle).expect("Failed to create connection");
    let adapter = connection
        .create_adapter()
        .expect("Failed to create adapter");

    let inner_size = window.inner_size();
    let surface_size = glutin_size_to_euclid_size(inner_size).to_i32().to_untyped();
    let rendering_context = RenderingContext::create(&connection, &adapter, Some(surface_size))
        .expect("Failed to create WR surfman");

    let viewport_origin = DeviceIntPoint::zero(); // bottom left
    let viewport_size = glutin_size_to_euclid_size(window.inner_size()).to_f32();
    let viewport = DeviceIntRect::from_origin_and_size(viewport_origin, viewport_size.to_i32());

    let app_window = Rc::new(Window {
        animating: Cell::new(false),
        coordinates: RefCell::new(EmbedderCoordinates {
            hidpi_factor: Scale::new(window.scale_factor() as f32),
            screen_size: viewport.size().cast_unit(),
            available_screen_size: viewport.size().cast_unit(),
            window_rect: viewport.cast_unit(),
            framebuffer: viewport.size(),
            viewport,
        }),
    });
    let opts = Opts::default();
    let prefs = Preferences::default();
    let mut servo = Servo::new(
        opts,
        prefs,
        rendering_context.clone(),
        Box::new(Embedder {
            waker: Waker(proxy),
        }),
        app_window.clone(),
        None,
        CompositeTarget::Window,
    );
    let browser_id = WebViewId::new();
    servo.setup_logging();
    servo.handle_events(vec![EmbedderEvent::NewWebView(
        ServoUrl::parse("http://neverssl.com").unwrap(),
        browser_id,
    )]);

    let mut wrapped_context = unsafe {
        rendering_context.device().create_context_from_native_context(
            surfman::NativeContext(windowed_context.context().raw_handle()),
        ).unwrap()
    };

    let windowed_context = RefCell::new(Some(windowed_context));

    let mut servo = Some(servo);
    let mut cursor_pos = Point2D::zero();

    el.run(move |event, _, control_flow| {
        //println!("{:?}", event);
        *control_flow = if app_window.animating.get() { ControlFlow::Poll } else { ControlFlow::Wait };
        let mut events = vec![];
        match event {
            Event::LoopDestroyed => {
                return;
            },
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::Resized(physical_size) => {
                        if physical_size.width as i32 > 0 && physical_size.height as i32 > 0 {
                            let new_size = Size2D::new(physical_size.width, physical_size.height).to_i32();
                            let viewport = DeviceIntRect::from_origin_and_size(viewport_origin, new_size.to_i32());
                            let mut coordinates = app_window.coordinates.borrow_mut();
                            coordinates.window_rect = viewport.cast_unit();
                            coordinates.viewport = viewport;
                            coordinates.framebuffer = viewport.size();
                            rendering_context.resize(new_size.to_untyped()).unwrap();
                            events.push(EmbedderEvent::MoveResizeWebView(browser_id, viewport.to_f32()));
                            events.push(EmbedderEvent::WindowResize);
                            let windowed_context2 = windowed_context.borrow_mut().take().unwrap();
                            windowed_context2.resize(physical_size);
                            *windowed_context.borrow_mut() = Some(windowed_context2);
                        }
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let position = glutin_position_to_euclid_point(position);
                        cursor_pos = position;
                        events.push(EmbedderEvent::MouseWindowMoveEventClass(position.to_f32()));
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == MouseButton::Left {
                            match state {
                                ElementState::Pressed => {
                                    events.push(
                                        EmbedderEvent::MouseWindowEventClass(
                                            MouseWindowEvent::MouseDown(
                                                ServoMouseButton::Left,
                                                cursor_pos.to_f32(),
                                            )
                                        )
                                    );
                                }
                                ElementState::Released => {
                                    events.push(
                                        EmbedderEvent::MouseWindowEventClass(
                                            MouseWindowEvent::MouseUp(
                                                ServoMouseButton::Left,
                                                cursor_pos.to_f32(),
                                            )
                                        )
                                    );

                                    events.push(
                                        EmbedderEvent::MouseWindowEventClass(
                                            MouseWindowEvent::Click(
                                                ServoMouseButton::Left,
                                                cursor_pos.to_f32(),
                                            )
                                        )
                                    );
                                }
                            }
                        }
                    }
                    _ => (),
                }
            },
            Event::RedrawRequested(_) => {
                servo.as_mut().unwrap().present();
                let windowed_context2 = windowed_context.borrow_mut().take().unwrap();
                let size = windowed_context2.window().inner_size().cast::<i32>();
                let windowed_context2 = unsafe { windowed_context2.make_current() }.unwrap();
                rendering_context.with_front_buffer(|device, surface| {
                    let info = device.surface_info(&surface);
                    let texture = device.create_surface_texture(&mut wrapped_context, surface).unwrap();
                    let texture_id = device.surface_texture_object(&texture);
                    gl.draw_texture(device, texture_id, info.size, Size2D::new(size.width, size.height));
                    let surface = device.destroy_surface_texture(&mut wrapped_context, texture).unwrap();
                    surface
                });
                windowed_context2.swap_buffers().unwrap();
                *windowed_context.borrow_mut() = Some(windowed_context2);
                
            }
            Event::UserEvent(()) => {
                events.push(EmbedderEvent::Idle);
                windowed_context.borrow().as_ref().unwrap().window().request_redraw();
            }
            _ => (),
        }

        let mut need_present = app_window.animating.get();

        loop {
            if servo.is_none() {
                break;
            }
            let mut shutting_down = false;
            need_present |= servo.as_mut().unwrap().handle_events(events.drain(..));

            let servo_events = servo.as_mut().unwrap().get_events();
            if servo_events.len() == 0 {
                break;
            }
            for (webview_id, event) in servo_events {
                if !matches!(event, EmbedderMsg::EventDelivered(CompositorEventVariant::MouseMoveEvent)) {
                    println!("{:?}", (webview_id, &event));
                }
                if let EmbedderMsg::ReadyToPresent(_) = event {
                    need_present |= true;
                    windowed_context.borrow().as_ref().unwrap().window().request_redraw();
                }
                if let EmbedderMsg::Shutdown = event {
                    shutting_down = true;
                    break;
                }
                if let EmbedderMsg::AllowNavigationRequest(id, ..) = event {
                    events.push(EmbedderEvent::AllowNavigationResponse(id, true));
                }
                if let EmbedderMsg::SetCursor(cursor) = event {
                    let windowed_context2 = windowed_context.borrow_mut().take().unwrap();
                    let window = windowed_context2.window();
                    if let Some(cursor) = match cursor {
                        Cursor::None => Some(CursorIcon::Default),
                        Cursor::Pointer => Some(CursorIcon::Hand),
                        Cursor::Text => Some(CursorIcon::Text),
                        _ => None,
                    } {
                        window.set_cursor_icon(cursor);
                    }
                    *windowed_context.borrow_mut() = Some(windowed_context2);
                }
                if let EmbedderMsg::WebViewOpened(new_webview_id) = event {
                    let rect = app_window.get_coordinates().get_viewport().to_f32();
                    events.push(EmbedderEvent::FocusWebView(new_webview_id));
                    events.push(EmbedderEvent::MoveResizeWebView(new_webview_id, rect));
                    events.push(EmbedderEvent::RaiseWebViewToTop(new_webview_id, true));
                }
            }

            if shutting_down {
                let servo = servo.take().unwrap();
                servo.deinit();
                control_flow.set_exit();
                break;
            }
        }

        if need_present {
            servo.as_mut().unwrap().present();
        }
    });
}
