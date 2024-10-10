use crate::package::{Package, WithFile};
use askama::Template;

#[derive(Template)]
#[template(path = "list-package.html")]
pub struct ListPackageTemplate<'a> {
    pub files: Vec<Package<'a, WithFile>>,
}
