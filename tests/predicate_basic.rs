use common::*;

#[test]
pub fn new_predicate_basic() {
  single(&|holmes: &mut Holmes| { holmes_exec!(holmes, { 
    predicate!(test_pred(string, blob, uint64))
  })})
}

#[test]
pub fn double_register() {
  single(&|holmes: &mut Holmes| { holmes_exec!(holmes, { 
    predicate!(test_pred(string, blob, uint64));
    predicate!(test_pred(string, blob, uint64))
  })})
}

#[test]
pub fn double_register_incompat() {
  single(&|holmes: &mut Holmes| { holmes_exec!(holmes, { 
    predicate!(test_pred(string, blob, uint64));
    should_fail(predicate!(test_pred(string, string, string)))
  })})
}

#[test]
pub fn pred_persist() {
  wrap(vec![&|holmes : &mut Holmes| {
    predicate!(holmes, test_pred(string, blob, uint64))
  }, &|holmes : &mut Holmes| {
    holmes_exec!(holmes, {
      should_fail(predicate!(test_pred(string, string, string)))
    })
  }]);
}
