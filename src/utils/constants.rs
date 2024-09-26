pub struct URL;

/// Just a collection of URLS
impl URL {
    pub const BASE: String = String::from("https://start.schulportal.hessen.de/");
    pub const LOGIN: String = String::from("https://login.schulportal.hessen.de/");
    pub const CONNECT: String = String::from("https://connect.schulportal.hessen.de/");
    pub const SCHOOLS: String = String::from("https://startcache.schulportal.hessen.de/exporteur.php");

    pub const START: String = String::from(URL::BASE + "startseite.php");
}