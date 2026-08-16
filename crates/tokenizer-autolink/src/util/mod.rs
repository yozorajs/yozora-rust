pub mod email;
pub mod uri;

pub use email::eat_email_address;
pub use uri::{eat_absolute_uri, eat_autolink_schema};
