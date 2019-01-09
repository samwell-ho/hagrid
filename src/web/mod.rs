use rocket;
use rocket::{State, Outcome};
use rocket::http::Status;
use rocket::request::{self, Request, FromRequest};
use rocket::response::status::Custom;
use rocket::response::NamedFile;
use rocket::fairing::AdHoc;

use rocket_contrib::templates::Template;
use std::path::{Path, PathBuf};

mod upload;

use database::{Polymorphic, Database};
use types::{Fingerprint, Email, KeyID};
use errors::Result;
use Opt;

use std::str::FromStr;
use std::result;

mod queries {
    use types::{Fingerprint, Email};

    #[derive(Debug)]
    pub enum Hkp {
        Fingerprint{ fpr: Fingerprint, index: bool },
        Email{ email: Email, index: bool },
    }
}

mod templates {
    #[derive(Serialize)]
    pub struct Verify {
        pub verified: bool,
        pub userid: String,
        pub fpr: String,
    }

    #[derive(Serialize)]
    pub struct Delete {
        pub token: String,
        pub fpr: String,
    }

    #[derive(Serialize)]
    pub struct Confirm {
        pub deleted: bool,
    }
}

struct StaticDir(String);
pub struct MailTemplateDir(String);
pub struct Domain(String);
pub struct From(String);

impl<'a, 'r> FromRequest<'a, 'r> for queries::Hkp {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<queries::Hkp, ()> {
        use rocket::request::FormItems;
        use std::collections::HashMap;

        let query = request.uri().query().unwrap_or("");
        let fields = FormItems::from(query).map(|item| {

            let (k, v) = item.key_value();

            let key = k.url_decode().unwrap_or_default();
            let value = v.url_decode().unwrap_or_default();
            (key, value)
        }).collect::<HashMap<_,_>>();

        if fields.len() >= 2 && fields.get("op").map(|x| x  == "get" || x == "index").unwrap_or(false) {
            let index = fields.get("op").map(|x| x == "index")
                .unwrap_or(false);
            let search = fields.get("search").cloned().unwrap_or_default();
            let maybe_fpr = Fingerprint::from_str(&search);

            if let Ok(fpr) = maybe_fpr {
                Outcome::Success(queries::Hkp::Fingerprint{
                    fpr: fpr, index: index
                })
            } else {
                match Email::from_str(&search) {
                    Ok(email) => Outcome::Success(queries::Hkp::Email{
                        email: email, index: index
                    }),
                    Err(_) => Outcome::Failure((Status::BadRequest, ())),
                }
            }
        } else {
            Outcome::Failure((Status::BadRequest, ()))
        }
    }
}

fn process_key(bytes: &[u8]) -> result::Result<String, Custom<String>> {
    use std::io::Write;
    use sequoia_openpgp::armor::{Writer, Kind};

    let key = || -> Result<String> {
        let mut buffer = Vec::default();
        {
            let mut writer = Writer::new(&mut buffer, Kind::PublicKey, &[])?;
            writer.write_all(&bytes)?;
        }

        Ok(String::from_utf8(buffer)?)
    }();

    match key {
        Ok(s) => Ok(s),
        Err(_) =>
            Err(Custom(Status::InternalServerError,
                       "Failed to ASCII armor key".to_string())),
    }
}

#[get("/by-fpr/<fpr>")]
fn by_fpr(db: rocket::State<Polymorphic>, fpr: String)
    -> result::Result<String, Custom<String>>
{
    let maybe_key = match Fingerprint::from_str(&fpr) {
        Ok(ref fpr) => db.by_fpr(fpr),
        Err(_) => None,
    };

    match maybe_key {
        Some(ref bytes) => process_key(bytes),
        None => Ok("No such key :-(".to_string()),
    }
}

#[get("/by-email/<email>")]
fn by_email(db: rocket::State<Polymorphic>, email: String)
    -> result::Result<String, Custom<String>>
{
    let maybe_key = match Email::from_str(&email) {
        Ok(ref email) => db.by_email(email),
        Err(_) => None,
    };

    match maybe_key {
        Some(ref bytes) => process_key(bytes),
        None => Ok("No such key :-(".to_string()),
    }
}

#[get("/by-kid/<kid>")]
fn by_kid(db: rocket::State<Polymorphic>, kid: String)
    -> result::Result<String, Custom<String>>
{
    let maybe_key = match KeyID::from_str(&kid) {
        Ok(ref key) => db.by_kid(key),
        Err(_) => None,
    };

    match maybe_key {
        Some(ref bytes) => process_key(bytes),
        None => Ok("No such key :-(".to_string()),
    }
}

#[get("/vks/verify/<token>")]
fn verify(db: rocket::State<Polymorphic>, token: String)
    -> result::Result<Template, Custom<String>>
{
    match db.verify_token(&token) {
        Ok(Some((userid, fpr))) => {
            let context = templates::Verify{
                verified: true,
                userid: userid.to_string(),
                fpr: fpr.to_string(),
            };

            Ok(Template::render("verify", context))
        }
        Ok(None) | Err(_) => {
            let context = templates::Verify{
                verified: false,
                userid: "".into(),
                fpr: "".into(),
            };

            Ok(Template::render("verify", context))
        }
    }
}

#[get("/vks/delete/<fpr>")]
fn delete(db: rocket::State<Polymorphic>, fpr: String,
          tmpl: State<MailTemplateDir>, domain: State<Domain>, from: State<From>)
    -> result::Result<Template, Custom<String>>
{
    use mail::send_confirmation_mail;

    let fpr = match Fingerprint::from_str(&fpr) {
        Ok(fpr) => fpr,
        Err(_) => {
            return Err(Custom(Status::BadRequest,
                              "Invalid fingerprint".to_string()));
        }
    };

    match db.request_deletion(fpr.clone()) {
        Ok((token,uids)) => {
            let context = templates::Delete{
                fpr: fpr.to_string(),
                token: token.clone(),
            };

            for uid in uids {
                send_confirmation_mail(&uid, &token, &tmpl.0, &domain.0, &from.0)
                    .map_err(|err| {
                        Custom(Status::InternalServerError,
                               format!("{:?}", err))
                    })?;
            }

            Ok(Template::render("delete", context))
        }
        Err(e) => Err(Custom(Status::InternalServerError,
                             format!("{}", e))),
    }
}

#[get("/vks/confirm/<token>")]
fn confirm(db: rocket::State<Polymorphic>, token: String)
    -> result::Result<Template, Custom<String>>
{
    match db.confirm_deletion(&token) {
        Ok(true) => {
            let context = templates::Confirm{
                deleted: true,
            };

            Ok(Template::render("confirm", context))
        }
        Ok(false) | Err(_) => {
            let context = templates::Confirm{
                deleted: false,
            };

            Ok(Template::render("confirm", context))
        }
    }
}

#[get("/assets/<file..>")]
fn files(file: PathBuf, static_dir: State<StaticDir>) -> Option<NamedFile> {
    NamedFile::open(Path::new(&static_dir.0).join("assets").join(file)).ok()
}

#[get("/pks/lookup")]
fn lookup(db: rocket::State<Polymorphic>, key: Option<queries::Hkp>)
    -> result::Result<String, Custom<String>>
{
    use std::io::Write;
    use sequoia_openpgp::RevocationStatus;
    use sequoia_openpgp::{TPK, parse::Parse};
    use sequoia_openpgp::armor::{Writer, Kind};

    let (maybe_key,index) = match key {
        Some(queries::Hkp::Fingerprint{ ref fpr, index }) => {
            (db.by_fpr(fpr),index)
        }
        Some(queries::Hkp::Email{ ref email, index }) => {
            (db.by_email(email),index)
        }
        None => { return Ok("nothing to do".to_string()); }
    };

    match maybe_key {
        Some(ref bytes) if !index => {
            let key = || -> Result<String> {
                let mut buffer = Vec::default();
                {
                    let mut writer = Writer::new(&mut buffer, Kind::PublicKey,
                                                 &[])?;
                    writer.write_all(&bytes)?;
                }

                Ok(String::from_utf8(buffer)?)
            }();

            match key {
                Ok(s) => Ok(s),
                Err(_) =>
                    Err(Custom(Status::InternalServerError,
                               "Failed to ASCII armor key".to_string())),
            }
        }
        None if !index => Ok("No such key :-(".to_string()),

        Some(ref bytes) if index => {
            let tpk = TPK::from_bytes(bytes)
                .map_err(|e| Custom(Status::InternalServerError,
                                    format!("{}", e)))?;
            let mut out = String::default();
            let p = tpk.primary();

            let ctime = tpk
                .primary_key_signature()
                .and_then(|x| x.signature_creation_time())
                .map(|x| format!("{}", x.to_timespec().sec))
                .unwrap_or_default();
            let extime = tpk
                .primary_key_signature()
                .and_then(|x| x.signature_expiration_time())
                .map(|x| format!("{}", x))
                .unwrap_or_default();
            let is_exp = tpk
                .primary_key_signature()
                .and_then(|x| {
                    if x.signature_expired() { "e" } else { "" }.into()
                })
                .unwrap_or_default();
            let is_rev =
                if tpk.revoked() != RevocationStatus::NotAsFarAsWeKnow {
                    "r"
                } else {
                    ""
                };
            let algo: u8 = p.pk_algo().into();

            out.push_str("info:1:1\r\n");
            out.push_str(&format!("pub:{}:{}:{}:{}:{}:{}{}\r\n",
                                 p.fingerprint().to_string().replace(" ",""),
                                 algo,
                                 p.mpis().bits(),
                                 ctime,extime,is_exp,is_rev));

            for uid in tpk.userids() {
                let u =
                    url::form_urlencoded::byte_serialize(uid.userid().userid())
                    .fold(String::default(),|acc,x| acc + x);
                let ctime = uid
                    .binding_signature()
                    .and_then(|x| x.signature_creation_time())
                    .map(|x| format!("{}", x.to_timespec().sec))
                    .unwrap_or_default();
                let extime = uid
                    .binding_signature()
                    .and_then(|x| x.signature_expiration_time())
                    .map(|x| format!("{}", x))
                    .unwrap_or_default();
                let is_exp = uid
                    .binding_signature()
                    .and_then(|x| {
                        if x.signature_expired() { "e" } else { "" }.into()
                    })
                    .unwrap_or_default();
                let is_rev =
                    if uid.revoked() != RevocationStatus::NotAsFarAsWeKnow {
                        "r"
                    } else {
                        ""
                    };

                out.push_str(&format!("uid:{}:{}:{}:{}{}\r\n",
                                      u,ctime,extime,is_exp,is_rev));
            }
            Ok(out)
        }
        None if index => {
            Ok("info:1:0\r\n".into())
        }

        _ => unreachable!(),
    }
}

#[get("/")]
fn root() -> Template {
    use std::collections::HashMap;

    Template::render("index", HashMap::<String, String>::default())
}

pub fn serve(opt: &Opt, db: Polymorphic) -> Result<()> {
    use rocket::config::{Config, Environment};
    use std::str::FromStr;

    let (addr, port) = match opt.listen.find(':') {
        Some(p) => {
            let addr = opt.listen[0..p].to_string();
            let port = if p < opt.listen.len() - 1 {
                u16::from_str(&opt.listen[p+1..]).ok().unwrap_or(8080)
            } else {
                8080
            };

            (addr, port)
        }
        None => (opt.listen.to_string(), 8080)
    };

    let config = Config::build(Environment::Staging)
        .address(addr)
        .port(port)
        .workers(2)
        .root(opt.base.clone())
        .extra("template_dir", opt.base.join("templates").to_str()
               .ok_or("Template path invalid")?)
        .extra("static_dir", opt.base.join("public").to_str()
               .ok_or("Static path invalid")?)
        .extra("domain", opt.domain.clone())
        .extra("from", opt.from.clone())
        .finalize()?;
    let routes = routes![
        // infra
        root,
        files,
        // nginx-supported lookup
        by_email,
        by_fpr,
        by_kid,
        // HKP
        lookup,
        upload::multipart_upload,
        // verification & deletion
        verify,
        delete,
        confirm,
    ];

    rocket::custom(config)
        .attach(Template::fairing())
        .attach(AdHoc::on_attach("static_dir", |rocket| {
            let static_dir = rocket.config()
                .get_str("static_dir")
                .unwrap()
                .to_string();

            Ok(rocket.manage(StaticDir(static_dir)))
        }))
        .attach(AdHoc::on_attach("template_dir", |rocket| {
            let static_dir = rocket.config()
                .get_str("template_dir")
                .unwrap()
                .to_string();

            Ok(rocket.manage(MailTemplateDir(static_dir)))
        }))
        .attach(AdHoc::on_attach("domain", |rocket| {
            let domain = rocket.config()
                .get_str("domain")
                .unwrap()
                .to_string();

            Ok(rocket.manage(Domain(domain)))
        }))
        .attach(AdHoc::on_attach("from", |rocket| {
            let from = rocket.config()
                .get_str("from")
                .unwrap()
                .to_string();

            Ok(rocket.manage(From(from)))
        }))

        .mount("/", routes)
        .manage(db)
        .launch();
    Ok(())
}
