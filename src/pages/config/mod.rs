//! Editor parts shared by the node settings page (a local profile) and the console's network editor (a config on an
//! easytier-web server): the core's behavior switches and the access control list. Both hosts hold the values (a
//! draft, a form) and pass accessors, so these parts keep no copy of what they edit.

mod acl;
mod draft;
mod flags;
mod groups;
mod ops;
mod rules;
mod text;

pub(in crate::pages) use acl::acl_editor;
pub(in crate::pages) use flags::flag_switches;
pub(in crate::pages) use text::acl_error_text;
