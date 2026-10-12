//! Against a Db2 server named by ZEUS_DB2 (`db2://user:password@host:port/DB`,
//! see tools/db2ce.sh); skipped when it is not set.

use zeus_drda::sql::Value;
use zeus_drda::{Connection, Params};

fn params() -> Option<Params> {
    let url = std::env::var("ZEUS_DB2").ok()?;
    Params::from_url(&url).ok()
}

#[test]
fn statements_and_queries() {
    let Some(p) = params() else {
        eprintln!("ZEUS_DB2 not set; skipped");
        return;
    };
    let mut c = Connection::connect(&p).unwrap();
    let _ = c.execute("DROP TABLE ZEUSTEST.T1");
    let ca = c
        .execute("CREATE TABLE ZEUSTEST.T1 (ID INTEGER NOT NULL, NAME VARCHAR(20), AMT DECIMAL(9,2), CD CHAR(3))")
        .unwrap();
    assert_eq!(ca.sqlcode, 0, "{ca:?}");
    for i in 1..=600 {
        let ca = c
            .execute(&format!(
                "INSERT INTO ZEUSTEST.T1 VALUES ({i}, 'N{i}', {i}.25, NULL)"
            ))
            .unwrap();
        assert_eq!(ca.errd[2], 1);
    }
    c.commit().unwrap();
    let r = c
        .query("SELECT ID, NAME, AMT, CD FROM ZEUSTEST.T1 ORDER BY ID")
        .unwrap();
    assert_eq!(r.rows.len(), 600);
    assert_eq!(
        r.rows[599],
        vec![
            Value::Int(600),
            Value::Str("N600".into()),
            Value::Dec("600.25".into()),
            Value::Null
        ]
    );
    // a statement error comes back in the SQLCA
    let ca = c.execute("INSERT INTO ZEUSTEST.T1 VALUES ('X')").unwrap();
    assert!(ca.sqlcode < 0);
    let r = c.query("SELECT * FROM ZEUSTEST.NOSUCH").unwrap();
    assert_eq!(r.sqlca.sqlcode, -204);
    // rollback undoes
    c.execute("DELETE FROM ZEUSTEST.T1").unwrap();
    c.rollback().unwrap();
    let r = c.query("SELECT COUNT(*) FROM ZEUSTEST.T1").unwrap();
    assert_eq!(r.rows[0][0], Value::Int(600));
    c.execute("DROP TABLE ZEUSTEST.T1").unwrap();
    c.commit().unwrap();
}
