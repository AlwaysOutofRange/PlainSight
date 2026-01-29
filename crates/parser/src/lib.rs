pub mod framework;
pub mod languages;
pub mod registry;

use registry::Registry;

pub fn default_registry() -> Registry {
    Registry::new(vec![
        Box::new(languages::java::JavaAdapter),
        Box::new(languages::empty::EmptyAdapter),
    ])
}
