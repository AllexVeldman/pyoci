use crate::package::Info;
use askama::Template;

#[derive(Template)]
#[template(path = "list-package.html")]
pub struct ListPackageTemplate {
    pub host: url::Url,
    pub files: Vec<Info>,
}
