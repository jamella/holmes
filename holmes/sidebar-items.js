initSidebarItems({"enum":[["DB","Defines the database connection parameters"],["Error","Ways that a `Holmes` operation might go wrong"]],"macro":[["bind_match!","Constructs a bind match outer object."],["clause_match!","Generates a `MatchExpr` from a representation"],["fact!","Stores a fact with the `Holmes` context."],["func!","Registers a native rust function with the `Holmes` object for use in rules."],["hexpr!","Generates an expression structure"],["holmes_exec!","Shorthand notation for performing many actions with the same holmes context Analogous to a weaker version of the `Reader` monad which cannot return values."],["htype!","Converts an EDSL type specification into a Holmes type object Takes the name of a variable containing a holmes object as the first parameter, and a type description as the second."],["predicate!","Registers a predicate with the `Holmes` context."],["query!","Runs a datalog query against the `Holmes` context"],["rule!","Adds a Holmes rule to the system"],["typed_unpack!","Given a value and a type it is believed to be, unpack it to the greatest extent possible (e.g. unpack through tupling and lists)"],["typet_boiler!",""],["typet_inner!",""],["typet_inner_eq!",""],["valuet_boiler!",""]],"mod":[["edsl","Holmes EDSL"],["engine","Holmes/Datalog Execution Engine"],["fact_db","This module defines the interface which a fact database must present to be used as a backend by the Holmes engine."],["mem_db","This is a memory mock for the fact database interface."],["pg","Postgres-based Fact Database"]],"struct":[["Holmes","Encapsulates the user-level interface to a holmes database"]],"type":[["Result","`Result` is a shorthand type for the standard `Result` specialized to our `Error type."]]});