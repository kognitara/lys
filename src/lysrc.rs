use std::net::{SocketAddr, TcpListener};

use serde::Deserialize;
use tabled::Tabled;

#[derive(Default, Deserialize, Tabled)]
pub struct Lysrc {
    title: String,
    description: String,
    footer: String,
    favicon: String,
    homepage: String,
    documentation: String,
    port: u16,
}

impl Lysrc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_title(&mut self, title: &str) -> &mut Self {
        self.title.clear();
        self.title.push_str(title);
        self
    }

    pub fn get_title(&self) -> String {
        self.title.clone()
    }

    pub fn get_description(&self) -> String {
        self.description.clone()
    }

    pub fn get_footer(&self) -> String {
        self.footer.clone()
    }

    pub fn get_favicon(&self) -> String {
        self.favicon.clone()
    }

    pub fn get_documentation(&self) -> String {
        self.documentation.clone()
    }

    pub fn get_homepage(&self) -> String {
        self.homepage.clone()
    }

    pub fn get_port(&self) -> u16 {
        self.port.clone()
    }

    pub fn set_description(&mut self, description: &str) -> &mut Self {
        self.description.clear();
        self.description.push_str(description);
        self
    }

    pub fn set_footer(&mut self, footer: &str) -> &mut Self {
        self.footer.clear();
        self.footer.push_str(footer);
        self
    }

    pub fn set_favicon(&mut self, favicon: &str) -> &mut Self {
        self.favicon.clear();
        self.favicon.push_str(favicon);
        self
    }

    pub fn set_documentation(&mut self, documentation: &str) -> &mut Self {
        self.documentation.clear();
        self.documentation.push_str(documentation);
        self
    }

    pub fn set_homepage(&mut self, homepage: &str) -> &mut Self {
        self.homepage.clear();
        self.homepage.push_str(homepage);
        self
    }

    pub fn set_port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }

    pub fn available(&self, addr: SocketAddr) -> bool {
        TcpListener::bind(addr).is_ok()
    }
}

#[cfg(test)]
mod test {
    use crate::lysrc::Lysrc;
    use std::net::SocketAddr;

    #[test]
    pub fn it_works() {
        let mut lysrc = Lysrc::new();
        lysrc
            .set_title("Lys")
            .set_description("a modern vcs")
            .set_favicon("favicon.ico")
            .set_homepage("https://localhost")
            .set_documentation("https://localhost:7789")
            .set_footer("lysrc")
            .set_port(7789);
        assert_eq!("Lys", &lysrc.get_title());
        assert_eq!("a modern vcs", &lysrc.get_description());
        assert_eq!("favicon.ico", &lysrc.get_favicon());
        assert_eq!("https://localhost", &lysrc.get_homepage());
        assert_eq!("https://localhost:7789", &lysrc.get_documentation());
        assert_eq!("lysrc", &lysrc.get_footer());
        assert_eq!(7789, lysrc.get_port());
        let addr = SocketAddr::from(([0, 0, 0, 0], lysrc.get_port()));
        assert_eq!(true, lysrc.available(addr));
    }
}
