/// Where the Scout tab can navigate.
struct ProjectRoute: Hashable {
  let id: String
  let title: String
}

struct TaskRoute: Hashable {
  let project: String
  let task: String
  let title: String
}
