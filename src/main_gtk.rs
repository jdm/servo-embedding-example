use gio::prelude::*;
use gtk::prelude::*;
use simpleservo::*;
use std::env::args;

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("First GTK+ Program");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(350, 70);

    //let button = gtk::Button::with_label("Click me!");

    //window.add(&button);

    let glarea = gtk::GLArea::new();
    glarea.connect_realize(move |widget| {
        if widget.get_realized() {
	    widget.make_current();
        }
        let allocation = widget.get_allocation();

        let gl = gl_glue::gl::init().unwrap();
        let opts = InitOptions {
            args: vec![],
            coordinates: Coordinates::new(
                0, 0,
                allocation.width, allocation.height,
                allocation.width, allocation.height,
            ),
            density: 1.0,
            prefs: None,
            xr_discovery: None,
            surfman_integration: SurfmanIntegration::Surface,

            // only used for media hardware acceleration
            gl_context_pointer: None, 
            native_display_pointer: None,
        };

        //allocation.width,
        //allocation.height,
    });

    glarea.connect_resize(|_widget, _width, _height| {
    });

    glarea.connect_render(move |_widget, _gl_context| {
        Inhibit(false)
    });

    window.add(&glarea);

    window.show_all();
}

fn main() {
    let application =
        gtk::Application::new(Some("com.github.gtk-rs.examples.basic"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&args().collect::<Vec<_>>());
}
