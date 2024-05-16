use askama::Template;
use pyoci::package::Info;

#[derive(Template)]
#[template(path = "list-package.html")]
pub struct ListPackageTemplate {
    pub host: url::Url,
    pub files: Vec<Info>,
}
