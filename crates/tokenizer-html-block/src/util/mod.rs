pub mod eat_html_attribute;
pub mod eat_html_tagname;

pub use eat_html_attribute::{eat_html_attribute, EatHtmlAttributeResult, RawHtmlAttribute};
pub use eat_html_tagname::eat_html_tag_name;
