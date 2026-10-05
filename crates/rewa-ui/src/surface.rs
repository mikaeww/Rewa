//! A widget that hands its whole snapshot to one painter closure, so the Rust
//! renderer draws every pixel itself, the way the Windows window does.

use std::cell::RefCell;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

type Painter = Box<dyn Fn(&gtk::Snapshot, f32, f32)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Surface {
        pub painter: RefCell<Option<Painter>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Surface {
        const NAME: &'static str = "RewaSurface";
        type Type = super::Surface;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Surface {}

    impl WidgetImpl for Surface {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (width, height) = (widget.width() as f32, widget.height() as f32);
            if let Some(painter) = self.painter.borrow().as_ref() {
                painter(snapshot, width, height);
            }
        }
    }
}

glib::wrapper! {
    pub struct Surface(ObjectSubclass<imp::Surface>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Surface {
    pub fn new() -> Self {
        let surface: Self = glib::Object::new();
        surface.set_focusable(true);
        surface.set_hexpand(true);
        surface.set_vexpand(true);
        surface
    }

    pub fn set_painter(&self, painter: impl Fn(&gtk::Snapshot, f32, f32) + 'static) {
        self.imp().painter.replace(Some(Box::new(painter)));
    }
}
