use native_types::*;

pub enum PredResponse {
  PredicateCreated,
  PredicateExists,
  PredicateTypeMismatch,
  PredicateInvalid(String),
  PredFail(String)
}

pub enum FactResponse {
  FactCreated,
  FactExists,
  FactTypeMismatch,
  FactPredUnreg(String),
  FactFail(String)
}

pub enum SearchResponse<'a> {
  SearchNone,
  SearchAns(Vec<Vec<HValue<'a>>>),
  SearchInvalid,
  SearchFail(String)
}

pub trait FactDB: Send {
  fn new_predicate(&mut self, pred : Predicate) -> PredResponse;
  fn new_fact(&mut self, fact : &Fact) -> FactResponse;
  fn search_facts(&self, query : Vec<Clause>) -> SearchResponse;
}
