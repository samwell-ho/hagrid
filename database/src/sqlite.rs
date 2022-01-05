use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::{
    create_dir_all, read_link, remove_file, rename, set_permissions, File,
    OpenOptions, Permissions,
};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use pathdiff::diff_paths;
use std::time::SystemTime;
use tempfile;
use url::form_urlencoded;

use sync::FlockMutexGuard;
use types::{Email, Fingerprint, KeyID};
use Result;
use {Database, Query};

use wkd;

use openpgp::Cert;
use openpgp_utils::POLICY;

use r2d2_sqlite::rusqlite::params;
use r2d2_sqlite::SqliteConnectionManager;

pub struct Sqlite {
    pool: r2d2::Pool<SqliteConnectionManager>,

    keys_dir_log: PathBuf,
    dry_run: bool,
}

impl Sqlite {
    pub fn new_file(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let base_dir: PathBuf = base_dir.into();

        let db_file = base_dir.join("keys.sqlite");
        let manager = SqliteConnectionManager::file(db_file);

        Self::new_internal(base_dir, manager)
    }

    pub fn new_memory(base_dir: impl Into<PathBuf>) -> Result<Self> {
        let base_dir: PathBuf = base_dir.into();

        let manager = SqliteConnectionManager::memory();

        Self::new_internal(base_dir, manager)
    }

    #[cfg(test)]
    fn build_pool(
        manager: SqliteConnectionManager,
    ) -> Result<r2d2::Pool<SqliteConnectionManager>> {

        #[derive(Copy, Clone, Debug)]
        pub struct LogConnectionCustomizer;
        impl<E> r2d2::CustomizeConnection<rusqlite::Connection, E>
            for LogConnectionCustomizer
        {
            fn on_acquire(
                &self,
                conn: &mut rusqlite::Connection,
            ) -> std::result::Result<(), E> {
                println!("Acquiring sqlite pool connection: {:?}", conn);
                conn.trace(Some(|query| {
                    println!("{}", query);
                }));
                std::result::Result::Ok(())
            }

            fn on_release(&self, conn: rusqlite::Connection) {
                println!("Releasing pool connection: {:?}", conn);
            }
        }

        Ok(r2d2::Pool::builder()
            .max_size(2)
            .connection_customizer(Box::new(LogConnectionCustomizer {}))
            .build(manager)?)
    }

    #[cfg(not(test))]
    fn build_pool(
        manager: SqliteConnectionManager,
    ) -> Result<r2d2::Pool<SqliteConnectionManager>> {
        Ok(r2d2::Pool::builder().max_size(2).build(manager)?)
    }

    fn new_internal(
        base_dir: PathBuf,
        manager: SqliteConnectionManager,
    ) -> Result<Self> {
        let keys_dir_log = base_dir.join("log");
        let dry_run = false;

        let pool = Self::build_pool(manager)?;
        let conn = pool.get()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS certs (
                fingerprint     TEXT NOT NULL PRIMARY KEY,
                full            BLOB NOT NULL,
                published       BLOB
            )",
            [],
        )?;

        Ok(Self { pool, keys_dir_log, dry_run })
    }

    fn read_from_path(
        &self,
        path: &Path,
        allow_internal: bool,
    ) -> Option<String> {
        todo!()
    }

    fn read_from_path_bytes(
        &self,
        path: &Path,
        allow_internal: bool,
    ) -> Option<Vec<u8>> {
        todo!()
    }

    /// Returns the Fingerprint the given path is pointing to.
    pub fn path_to_fingerprint(path: &Path) -> Option<Fingerprint> {
        todo!()
    }

    /// Returns the KeyID the given path is pointing to.
    fn path_to_keyid(path: &Path) -> Option<KeyID> {
        todo!()
    }

    /// Returns the Email the given path is pointing to.
    fn path_to_email(path: &Path) -> Option<Email> {
        todo!()
    }

    /// Returns the backing primary key fingerprint for any key path.
    pub fn path_to_primary(path: &Path) -> Option<Fingerprint> {
        todo!()
    }

    fn link_email_vks(&self, email: &Email, fpr: &Fingerprint) -> Result<()> {
        todo!()
    }

    fn link_email_wkd(&self, email: &Email, fpr: &Fingerprint) -> Result<()> {
        todo!()
    }

    fn unlink_email_vks(&self, email: &Email, fpr: &Fingerprint) -> Result<()> {
        todo!()
    }

    fn unlink_email_wkd(&self, email: &Email, fpr: &Fingerprint) -> Result<()> {
        todo!()
    }

    fn open_logfile(&self, file_name: &str) -> Result<File> {
        let file_path = self.keys_dir_log.join(file_name);
        Ok(OpenOptions::new().create(true).append(true).open(file_path)?)
    }

    fn perform_checks(
        &self,
        checks_dir: &Path,
        tpks: &mut HashMap<Fingerprint, Cert>,
        check: impl Fn(&Path, &Cert, &Fingerprint) -> Result<()>,
    ) -> Result<()> {
        // XXX: stub
        Ok(())
    }
}

impl Database for Sqlite {
    type MutexGuard = FlockMutexGuard;
    type TempCert = String;

    fn lock(&self) -> Result<Self::MutexGuard> {
        todo!()
    }

    fn write_to_temp(&self, content: &[u8]) -> Result<Self::TempCert> {
        todo!()
    }

    fn write_log_append(
        &self,
        filename: &str,
        fpr_primary: &Fingerprint,
    ) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let fingerprint_line =
            format!("{:010} {}\n", timestamp, fpr_primary.to_string());

        self.open_logfile(filename)?.write_all(fingerprint_line.as_bytes())?;

        Ok(())
    }

    fn move_tmp_to_full(
        &self,
        file: Self::TempCert,
        fpr: &Fingerprint,
    ) -> Result<()> {
        Ok(())
    }

    fn move_tmp_to_published(
        &self,
        file: Self::TempCert,
        fpr: &Fingerprint,
    ) -> Result<()> {
        Ok(())
    }

    fn move_tmp_to_published_wkd(
        &self,
        file: Option<Self::TempCert>,
        fpr: &Fingerprint,
    ) -> Result<()> {
        Ok(())
    }

    fn write_to_quarantine(
        &self,
        fpr: &Fingerprint,
        content: &[u8],
    ) -> Result<()> {
        Ok(())
    }

    fn check_link_fpr(
        &self,
        fpr: &Fingerprint,
        fpr_target: &Fingerprint,
    ) -> Result<Option<Fingerprint>> {
        Ok(None)
    }

    fn lookup_primary_fingerprint(&self, term: &Query) -> Option<Fingerprint> {
        None
    }

    fn link_email(&self, email: &Email, fpr: &Fingerprint) -> Result<()> {
        Ok(())
    }

    fn unlink_email(&self, email: &Email, fpr: &Fingerprint) -> Result<()> {
        Ok(())
    }

    fn link_fpr(
        &self,
        from: &Fingerprint,
        primary_fpr: &Fingerprint,
    ) -> Result<()> {
        Ok(())
    }

    fn unlink_fpr(
        &self,
        from: &Fingerprint,
        primary_fpr: &Fingerprint,
    ) -> Result<()> {
        Ok(())
    }

    // XXX: slow
    fn by_fpr_full(&self, fpr: &Fingerprint) -> Option<String> {
        None
    }

    // XXX: slow
    fn by_primary_fpr(&self, fpr: &Fingerprint) -> Option<String> {
        None
    }

    // XXX: slow
    fn by_fpr(&self, fpr: &Fingerprint) -> Option<String> {
        None
    }

    // XXX: slow
    fn by_email(&self, email: &Email) -> Option<String> {
        None
    }

    // XXX: slow
    fn by_email_wkd(&self, email: &Email) -> Option<Vec<u8>> {
        None
    }

    // XXX: slow
    fn by_kid(&self, kid: &KeyID) -> Option<String> {
        None
    }

    /// Checks the database for consistency.
    ///
    /// Note that this operation may take a long time, and is
    /// generally only useful for testing.
    fn check_consistency(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openpgp::cert::CertBuilder;
    use tempfile::TempDir;
    use test;

    const DATA_1: &str = "data, content doesn't matter";
    const DATA_2: &str = "other data, content doesn't matter";
    const FINGERPRINT_1: &str = "D4AB192964F76A7F8F8A9B357BD18320DEADFA11";

    fn open_db() -> (TempDir, Sqlite, PathBuf) {
        let tmpdir = TempDir::new().unwrap();

        let db = Sqlite::new_memory(tmpdir.path()).unwrap();
        let log_path = db.keys_dir_log.join(db.get_current_log_filename());

        (tmpdir, db, log_path)
    }

    #[test]
    fn new() {
        let (_tmp_dir, db, _log_path) = open_db();
        let k1 = CertBuilder::new()
            .add_userid("a@invalid.example.org")
            .generate()
            .unwrap()
            .0;
        let k2 = CertBuilder::new()
            .add_userid("b@invalid.example.org")
            .generate()
            .unwrap()
            .0;
        let k3 = CertBuilder::new()
            .add_userid("c@invalid.example.org")
            .generate()
            .unwrap()
            .0;

        assert!(db.merge(k1).unwrap().into_tpk_status().email_status.len() > 0);
        assert!(
            db.merge(k2.clone()).unwrap().into_tpk_status().email_status.len()
                > 0
        );
        assert!(
            !db.merge(k2).unwrap().into_tpk_status().email_status.len() > 0
        );
        assert!(
            db.merge(k3.clone()).unwrap().into_tpk_status().email_status.len()
                > 0
        );
        assert!(
            !db.merge(k3.clone()).unwrap().into_tpk_status().email_status.len()
                > 0
        );
        assert!(
            !db.merge(k3).unwrap().into_tpk_status().email_status.len() > 0
        );
    }

    #[test]
    fn xx_by_fpr_full() -> Result<()> {
        let (_tmp_dir, db, _log_path) = open_db();
        let fpr1 = Fingerprint::from_str(FINGERPRINT_1)?;

        db.move_tmp_to_full(db.write_to_temp(DATA_1.as_bytes())?, &fpr1)?;
        db.link_fpr(&fpr1, &fpr1)?;

        assert_eq!(db.by_fpr_full(&fpr1).expect("must find key"), DATA_1);
        Ok(())
    }

    #[test]
    fn xx_by_kid() -> Result<()> {
        let (_tmp_dir, db, _log_path) = open_db();
        let fpr1 = Fingerprint::from_str(FINGERPRINT_1)?;

        db.move_tmp_to_full(db.write_to_temp(DATA_1.as_bytes())?, &fpr1)?;
        db.move_tmp_to_published(db.write_to_temp(DATA_2.as_bytes())?, &fpr1)?;
        db.link_fpr(&fpr1, &fpr1)?;

        assert_eq!(db.by_kid(&fpr1.into()).expect("must find key"), DATA_2);
        Ok(())
    }

    #[test]
    fn xx_by_primary_fpr() -> Result<()> {
        let (_tmp_dir, db, _log_path) = open_db();
        let fpr1 = Fingerprint::from_str(FINGERPRINT_1)?;

        db.move_tmp_to_full(db.write_to_temp(DATA_1.as_bytes())?, &fpr1)?;
        db.move_tmp_to_published(db.write_to_temp(DATA_2.as_bytes())?, &fpr1)?;

        assert_eq!(db.by_primary_fpr(&fpr1).expect("must find key"), DATA_2);
        Ok(())
    }

    #[test]
    fn uid_verification() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_uid_verification(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn uid_deletion() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_uid_deletion(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn subkey_lookup() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_subkey_lookup(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn kid_lookup() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_kid_lookup(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn upload_revoked_tpk() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_upload_revoked_tpk(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn uid_revocation() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_uid_revocation(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn regenerate() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_regenerate(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn key_reupload() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_reupload(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn uid_replacement() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_uid_replacement(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn uid_unlinking() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_unlink_uid(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn same_email_1() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_same_email_1(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn same_email_2() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_same_email_2(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn same_email_3() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_same_email_3(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn same_email_4() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_same_email_4(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn no_selfsig() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_no_selfsig(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn bad_uids() {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::test_bad_uids(&mut db, &log_path);
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn reverse_fingerprint_to_path() {
        let tmpdir = TempDir::new().unwrap();
        let db = Sqlite::new_memory(tmpdir.path()).unwrap();

        let fp: Fingerprint =
            "CBCD8F030588653EEDD7E2659B7DD433F254904A".parse().unwrap();

        // XXX: fixme
        //assert_eq!(Sqlite::path_to_fingerprint(&db.link_by_fingerprint(&fp)),
        //           Some(fp.clone()));
        db.check_consistency().expect("inconsistent database");
    }

    #[test]
    fn attested_key_signatures() -> Result<()> {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::attested_key_signatures(&mut db, &log_path)?;
        db.check_consistency()?;
        Ok(())
    }

    #[test]
    fn nonexportable_sigs() -> Result<()> {
        let (_tmp_dir, mut db, log_path) = open_db();
        test::nonexportable_sigs(&mut db, &log_path)?;
        db.check_consistency()?;
        Ok(())
    }
}
