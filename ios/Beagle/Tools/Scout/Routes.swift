/// Where the Scout tab can navigate.
nonisolated struct ProjectRoute: Hashable {
  let id: String
  let title: String
}

nonisolated struct TaskRoute: Hashable {
  let project: String
  let task: String
  let title: String
}
