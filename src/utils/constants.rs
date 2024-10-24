pub struct URL;

/// Just a collection of URLS
impl URL {
    pub const BASE: &'static str = "https://start.schulportal.hessen.de/";
    pub const AJAX: &'static str = "https://start.schulportal.hessen.de/ajax.php";
    pub const LOGIN: &'static str = "https://login.schulportal.hessen.de/#";
    pub const LOGIN_AJAX: &'static str = "https://start.schulportal.hessen.de/ajax_login.php";
    pub const CONNECT: &'static str = "https://connect.schulportal.hessen.de/";
    pub const SCHOOLS: &'static str = "https://startcache.schulportal.hessen.de/exporteur.php";

    pub const START: &'static str = "https://start.schulportal.hessen.de/startseite.php";

    pub const USER_DATA: &'static str = "https://start.schulportal.hessen.de/benutzerverwaltung.php";
    pub const MEIN_UNTERRICHT: &'static str = "https://start.schulportal.hessen.de/meinunterricht.php";
}