#ifndef HOLMES_SERVER_PGDAL_H_
#define HOLMES_SERVER_PGDAL_H_

#include "dal.h"

#include <vector>
#include <set>
#include <atomic>
#include <mutex>

#include <kj/common.h>
#include <capnp/message.h>

#include <pqxx/pqxx>

#include "holmes.capnp.h"
#include "fact_util.h"

namespace holmes {

class PgDAL : public DAL {
  public:
    PgDAL() : conn() {initDB();}
    PgDAL(std::string connStr) : conn(connStr) {initDB();}
    size_t setFacts(capnp::List<Holmes::Fact>::Reader);
    std::vector<DAL::Context> getFacts(capnp::List<Holmes::FactTemplate>::Reader);
    bool addType(std::string name,
                 capnp::List<Holmes::HType>::Reader argTypes);
  private:
    std::mutex mutex;
    pqxx::connection conn;
    void initDB();
    std::map<std::string, std::vector<Holmes::HType::Reader>> types;
    typedef capnp::MallocMessageBuilder MMB;
    std::vector<kj::Own<MMB>> mbs;
    void registerPrepared(std::string, size_t);
    KJ_DISALLOW_COPY(PgDAL);
};

}

#endif
