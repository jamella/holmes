//! Postgres-based Fact Database
//!
//! # Design Notes
//!
//! ## Scope
//!
//! The general philsophy is that things having to do with persistence go here,
//! while things related to non-persistent components go in `holmes::engine`.
//!
//! In the long run, we would like to persist nearly everything in the
//! database, so that a client-server model can one bay restored. However,
//! in the short term this has little benefit, so only the items needing to
//! use SQL for efficient execution are being included.
//!
//! The biggest hurdle here is the persistence of code:
//!
//! * How do we store Types?
//! * How do we store bound functions?
//!
//! One possible long term answer is Cap'n' Proto `SturdyRef`s
//!
//! ## Other Databases
//!
//! For the moment, this is the only implementation, and there are no others
//! on the horizon, so this interface is not abstract.
//!
//! The only major hurdle to using another backend would be figuring out how
//! to make the `dyn` module abstract over databases.
use std::collections::hash_map::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};

use postgres::{rows, Connection, SslMode};
use postgres::types::{FromSql, ToSql};

use engine::types::{Fact, Predicate, MatchExpr, Clause};

mod error;
pub mod dyn;

pub use self::error::Error;
use self::dyn::types;
use self::dyn::{Type, Value};
use fact_db::{FactDB, Result};

/// An iterator over a `postgres::rows::Row`.
/// It does not implement the normal iter interface because it does not have
/// a set item type, but it implements a similar interface for ease of use.
pub struct RowIter<'a> {
  row : &'a rows::Row<'a>,
  index : usize
}

impl <'a> RowIter<'a> {
  /// Create a new row iterator starting at the beginning of the provided row
  pub fn new(row : &'a rows::Row) -> Self {
    RowIter {
      row   : row,
      index : 0
    }
  }
  /// Gets the next item in the row, using a `FromSql` instance to read it.
  /// If there is not a next item, returns `None`
  pub fn next<T>(&mut self) -> Option<T> where T : FromSql {
    let idx = self.index;
    self.index += 1;
    self.row.get(idx)
  }
}

/// Object representing a postgres-backed fact database instance
pub struct PgDB {
  conn              : Connection,
  pred_by_name      : HashMap<String, Predicate>,
  insert_by_name    : HashMap<String, String>,
  named_types       : HashMap<String, Type>
}

impl PgDB {
  /// Create a new PgDB object by passing in a Postgres connection string
  // TODO Add type parameters to call?
  // At the moment, persistence with custom types will result in failures
  // on a reconnect, so use a fresh database every time.
  // There's not a good way to persist custom types, so that fix will likely
  // come with optional parameters to seed types in at db startup.
  // TODO Should we be passing in a Connection object rather than a string?
  pub fn new(conn_str : &str) -> Result<PgDB> {
    let conn = try!(Connection::connect(conn_str, SslMode::None));

    // Create schemas
    try!(conn.execute("create schema if not exists facts", &[]));

    // Create Tables
    try!(conn.execute("create table if not exists predicates \
                       (pred_name varchar not null, \
                        ordinal int4 not null, \
                        type varchar not null)",
                      &[]));
    try!(conn.execute("create table if not exists rules \
                      (id serial, rule varchar not null)", &[]));

    // Create incremental PgDB object
    let mut db = PgDB {
      conn : conn,
      pred_by_name      : HashMap::new(),
      insert_by_name    : HashMap::new(),
      named_types       : types::default_types().iter().map(|type_| {
                            (type_.name().unwrap().to_owned(), type_.clone())
                          }).collect()
    };

    // Reload predicate cache
    {
      let pred_stmt = try!(db.conn.prepare(
        "select pred_name, type from predicates ORDER BY pred_name, ordinal"));
      let pred_types = try!(pred_stmt.query(&[]));
      for type_entry in pred_types.iter() {
        let name : String = type_entry.get(0);
        let h_type_str : String = type_entry.get(1);
        let h_type = match db.get_type(&h_type_str) {
          Some(ty) => ty,
          None => return Err(Box::new(
                               Error::Type(format!("Type not in registry: {}",
                                                   h_type_str))))
        };
        match db.pred_by_name.entry(name.clone()) {
          Vacant(entry) => {
            let mut types = Vec::new();
            types.push(h_type.clone());
            entry.insert(Predicate {
              name  : name.clone(),
              types : types
            });
          }
          Occupied(mut entry) => {
            entry.get_mut().types.push(h_type.clone());
          }
        }
      }
    }

    //Populate fact insert cache
    for pred in db.pred_by_name.clone().values() {
      &db.gen_insert_stmt(pred);
    }

    Ok(db)
  }

  // Generates a prebuilt insert statement for a given predicate, and stores
  // it in the cache so we don't have to rebuild it every time.
  // TODO: Is it possible for these to be stored prepared statements somehow?
  fn gen_insert_stmt(&mut self, pred : &Predicate) {
    let args : Vec<String> = pred.types.iter().enumerate().map(|(k,_)|{
      format!("${}", k + 1)
    }).collect();
    let stmt = format!("insert into facts.{} values ({})",
                       pred.name,
                       args.join(", "));
    self.insert_by_name.insert(pred.name.clone(), stmt);
  }

  // Persist a predicate into the database
  // This function is internal because it does not add it to the object, it
  // _only_ puts record of the predicate into the database.
  fn insert_predicate(&self, pred : &Predicate) -> Result<()> {
    let &Predicate {ref name, ref types} = pred;
    for (ordinal, type_) in types.iter().enumerate() {
      try!(self.conn.execute("insert into predicates \
                              (pred_name, type, ordinal) \
                              values ($1, $2, $3)",
                             &[name,
                               &type_.name().unwrap(),
                               &(ordinal as i32)]));
    }
    let table_str = types.iter().flat_map(|type_| {type_.repr()}).enumerate()
                          .map(|(ord, repr)| {
                            format!("arg{} {}", ord, repr)
                          }).collect::<Vec<_>>().join(", ");
    try!(self.conn.execute(
           &format!("create table facts.{} ({})", name, table_str),
           &[]));
    Ok(())
  }
}
impl FactDB for PgDB {
  /// Adds a new fact to the database, returning false if the fact was already
  /// present in the database, and true if it was inserted.
  fn insert_fact(&mut self, fact : &Fact) -> Result<bool> {
    let stmt : String = try!(self.insert_by_name
      .get(&fact.pred_name)
      .ok_or(Error::Internal("Insert Statement Missing"
                           .to_string()))).clone();
    Ok(try!(self.conn.execute(&stmt,
                              &fact.args.iter().flat_map(|x|{
                                x.to_sql().into_iter()
                              }).collect::<Vec<_>>())) > 0)
  }

  /// Registers a new type with the database.
  /// This is unstable, and will likely need to be moved to the initialization
  /// of the database object in order to allow reconnecting to an existing
  /// database.
  fn add_type(&mut self, type_ : Type) -> Result<()> {
    let name = type_.name().unwrap();
    if !self.named_types.contains_key(name) {
      self.named_types.insert(name.to_owned(), type_.clone());
      Ok(())
    } else {
      Err(Box::new(Error::Type(format!("{} already registered", name))))
    }
  }

  /// Looks for a named type in the database's registry.
  /// This function is primarily useful for the DSL shorthand for constructing
  /// queries, since it allows you to use names of types when declaring
  /// functions rather than type objects.
  fn get_type(&self, type_str : &str) -> Option<Type> {
    self.named_types.get(type_str).map(|x|{x.clone()})
  }

  /// Fetches a predicate by name
  fn get_predicate(&self, pred_name : &str) -> Option<&Predicate> {
    self.pred_by_name.get(pred_name)
  }

  /// Persists a predicate by name
  /// The name *must* consist only of lower case ASCII and _, anything else
  /// will be rejected. This restriction is because the predicate name is
  /// currently used to construct the table name.
  ///
  /// In the future, this restriction could be lifted by generating table
  /// names rather than using the names of predicates, but this helps a lot
  /// with debugging for now.
  // TODO lift restriction on predicate names
  fn new_predicate(&mut self, pred : &Predicate) -> Result<()> {
    // The predicate name is used as a table name, check it for legality
    if !valid_name(&pred.name) {
      return Err(Box::new(Error::Arg(
              "Invalid name: Use lowercase and underscores only".to_string())))
    }
    // If this predicate was already registered, fail
    if self.pred_by_name.contains_key(&pred.name) {
      return Err(Box::new(Error::Arg(
              format!("Predicate {} already registered.", &pred.name))))
    }

    try!(self.insert_predicate(&pred));
    self.gen_insert_stmt(&pred);
    self.pred_by_name.insert(pred.name.clone(), pred.clone());
    Ok(())
  }

  /// Attempt to match the right hand side of a datalog rule against the
  /// database, returning a list of solution assignments to the bound
  /// variables.
  fn search_facts(&self, query : &Vec<Clause>)
    -> Result<Vec<Vec<Value>>> {
    // Check there is at least one clause
    if query.len() == 0 {
      return Err(Box::new(Error::Arg("Empty search query".to_string())));
    };

    // Check that clauses:
    // * Have sequential variables
    // * Reference predicates in the database
    // * Only unify variables of equal type
    {
      let mut var_type : Vec<Type> = Vec::new();
      for clause in query.iter() {
        let pred = match self.pred_by_name.get(&clause.pred_name) {
          Some(pred) => pred,
          None => return Err(Box::new(Error::Arg(
                  format!("{} is not a registered predicate.",
                          clause.pred_name)))),
        };
        for (idx, slot) in clause.args.iter().enumerate() {
          match *slot {
              MatchExpr::Unbound
            | MatchExpr::Const(_) => (),
              MatchExpr::Var(v) => {
                let v = v as usize;
                if v == var_type.len() {
                  var_type.push(pred.types[idx].clone())
                } else if v > var_type.len() {
                  return Err(Box::new(Error::Arg(format!("Hole between {} and {} in variable numbering.", var_type.len() - 1, v))))
                } else if var_type[v] != pred.types[idx].clone() {
                  return Err(Box::new(Error::Arg(format!("Variable {} attempt to unify incompatible types {:?} and {:?}", v, var_type[v], pred.types[idx]))))
                }
              }
          }
        }
      }
    }

    // Actually build and execute the query
    let mut tables = Vec::new();    // Predicate names involved in the query,
                                    // in the sequence they appear
    let mut restricts = Vec::new(); // Unification expressions, indexed by
                                    // which join they belong on.
    let mut var_names = Vec::new(); // Translation of variable numbers to
                                    // sql exprs
    let mut var_types = Vec::new(); // Translation of variable numbers to
                                    // Types
    let mut where_clause = Vec::new(); // Constant comparisons
    let mut vals : Vec<&ToSql> = Vec::new(); // Values to be quoted into the
                                             // prepared statement

    for (idxc, clause) in query.iter().enumerate() {
      // The clause refers to a table named by the predicate
      let table_name = format!("facts.{}", clause.pred_name);
      // We will refer to it by a numbered alias, to make joining easier
      let alias_name = format!("t{}", idxc);
      let mut clause_elements = Vec::new();
      for (idx, arg) in clause.args.iter().enumerate() {
        match arg {
          &MatchExpr::Unbound => (),
          &MatchExpr::Var(var) => if var >= var_names.len() {
              // This situation means it's the first occurrence of the variable
              // We record this definition as the canonical definition for use
              // in the select, and store the type to know how to extract it.
              var_names.push(
                format!("{}.arg{}", alias_name, idx));
              var_types.push(&self.pred_by_name[&clause.pred_name].types[idx]);
            } else {
              // The variable has occurred correctly, so we add it being equal
              // to the canonical definition to the join clause for this table
              let piece = format!("{}.arg{} = {}", alias_name, idx,
                                  var_names[var]);
              clause_elements.push(piece);
            },
          &MatchExpr::Const(ref val) => {
            // Since we're comparing against a constant, this restriction can
            // go in the where clause.
            // I stash the value in a buffer for later use with the prepared
            // statement, and put the index into the buffer into the where
            // clause chunk.
            vals.extend(val.to_sql());
            where_clause.push(
              format!("{}.arg{} = ${}", alias_name, idx, vals.len()));
          }
        }
      }
      restricts.push(clause_elements);
      tables.push(format!("{} as {}", table_name, alias_name));
    }
    // Make sure we're never empty on bound variables. If we are, we will get
    // SELECT FROM
    // which will not work.
    var_names.push("0".to_string());

    let vars = format!("{}", var_names.join(", "));
    tables.reverse();
    restricts.reverse();
    let main_table = tables.pop().unwrap();
    where_clause.append(&mut restricts.pop().unwrap());
    let join_query =
      tables.iter().zip(restricts.iter()).map(|(table, join)| {
        if join.len() == 0 {
          format!("JOIN {} ", table)
        } else {
          format!("JOIN {} ON {}", table, join.join(" AND "))
        }
      }).collect::<Vec<_>>().join(" ");
    let where_clause = {
      if where_clause.len() == 0 {
        String::new()
      } else {
        format!("WHERE {}", where_clause.join(" AND "))
      }
    };
    let raw_stmt =
      format!("SELECT {} FROM {} {} {}",
              vars, main_table, join_query,
              where_clause);
    let stmt = try!(self.conn.prepare(&raw_stmt));
    let rows = try!(stmt.query(&vals));

    let mut anss : Vec<Vec<Value>> = rows.iter().map(|row| {
      let mut row_iter = RowIter::new(&row);
      var_types.iter().map(|type_| {
        type_.extract(&mut row_iter)
      }).collect()
    }).collect();

    // TODO: Understand why this is necessary, if it should be necessary.
    anss.dedup();
    Ok(anss)
  }
}

fn valid_name(name : &String) -> bool {
  name.chars().all( |ch| match ch { 'a'...'z' | '_' => true, _ => false } )
}
