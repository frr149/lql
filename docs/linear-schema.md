# Linear GraphQL Schema Reference (auto-generated)
# Generated: 2026-03-26
# Source: Linear API introspection


## IssueFilter (INPUT_OBJECT)
  id: IssueIDComparator
  createdAt: DateComparator
  updatedAt: DateComparator
  number: NumberComparator
  title: StringComparator
  description: NullableStringComparator
  priority: NullableNumberComparator
  estimate: EstimateComparator
  startedAt: NullableDateComparator
  triagedAt: NullableDateComparator
  completedAt: NullableDateComparator
  canceledAt: NullableDateComparator
  archivedAt: NullableDateComparator
  autoClosedAt: NullableDateComparator
  autoArchivedAt: NullableDateComparator
  addedToCycleAt: NullableDateComparator
  addedToCyclePeriod: CyclePeriodComparator
  dueDate: NullableTimelessDateComparator
  accumulatedStateUpdatedAt: NullableDateComparator
  snoozedUntilAt: NullableDateComparator
  assignee: NullableUserFilter
  delegate: NullableUserFilter
  lastAppliedTemplate: NullableTemplateFilter
  recurringIssueTemplate: NullableTemplateFilter
  sourceMetadata: SourceMetadataComparator
  creator: NullableUserFilter
  parent: NullableIssueFilter
  snoozedBy: NullableUserFilter
  labels: IssueLabelCollectionFilter
  subscribers: UserCollectionFilter
  hasSharedUsers: RelationExistsComparator
  sharedWith: UserCollectionFilter
  team: TeamFilter
  projectMilestone: NullableProjectMilestoneFilter
  comments: CommentCollectionFilter
  activity: ActivityCollectionFilter
  suggestions: IssueSuggestionCollectionFilter
  cycle: NullableCycleFilter
  project: NullableProjectFilter
  state: WorkflowStateFilter
  children: IssueCollectionFilter
  attachments: AttachmentCollectionFilter
  searchableContent: ContentComparator
  hasRelatedRelations: RelationExistsComparator
  hasDuplicateRelations: RelationExistsComparator
  hasBlockedByRelations: RelationExistsComparator
  hasBlockingRelations: RelationExistsComparator
  hasSuggestedRelatedIssues: RelationExistsComparator
  hasSuggestedSimilarIssues: RelationExistsComparator
  hasSuggestedAssignees: RelationExistsComparator
  hasSuggestedProjects: RelationExistsComparator
  hasSuggestedLabels: RelationExistsComparator
  hasSuggestedTeams: RelationExistsComparator
  slaStatus: SlaStatusComparator
  reactions: ReactionCollectionFilter
  needs: CustomerNeedCollectionFilter
  releases: ReleaseCollectionFilter
  customerCount: NumberComparator
  customerImportantCount: NumberComparator
  leadTime: NullableDurationComparator
  cycleTime: NullableDurationComparator
  ageTime: NullableDurationComparator
  triageTime: NullableDurationComparator
  and: [IssueFilter]
  or: [IssueFilter]

## IssueCreateInput (INPUT_OBJECT)
  id: String
  title: String
  description: String
  descriptionData: JSON
  assigneeId: String
  delegateId: String
  parentId: String
  priority: Int
  estimate: Int
  subscriberIds: [String]
  labelIds: [String]
  teamId: String!
  cycleId: String
  projectId: String
  projectMilestoneId: String
  lastAppliedTemplateId: String
  stateId: String
  referenceCommentId: String
  sourceCommentId: String
  sourcePullRequestCommentId: String
  sortOrder: Float
  prioritySortOrder: Float
  subIssueSortOrder: Float
  dueDate: TimelessDate
  createAsUser: String
  displayIconUrl: String
  preserveSortOrderOnCreate: Boolean
  createdAt: DateTime
  slaBreachesAt: DateTime
  slaStartedAt: DateTime
  templateId: String
  completedAt: DateTime
  slaType: SLADayCountType
  useDefaultTemplate: Boolean

## IssueUpdateInput (INPUT_OBJECT)
  title: String
  description: String
  descriptionData: JSON
  assigneeId: String
  delegateId: String
  parentId: String
  priority: Int
  estimate: Int
  subscriberIds: [String]
  labelIds: [String]
  addedLabelIds: [String]
  removedLabelIds: [String]
  teamId: String
  cycleId: String
  projectId: String
  projectMilestoneId: String
  lastAppliedTemplateId: String
  stateId: String
  sortOrder: Float
  prioritySortOrder: Float
  subIssueSortOrder: Float
  dueDate: TimelessDate
  trashed: Boolean
  slaBreachesAt: DateTime
  slaStartedAt: DateTime
  snoozedUntilAt: DateTime
  snoozedById: String
  slaType: SLADayCountType
  autoClosedByParentClosing: Boolean

## IssueRelationCreateInput (INPUT_OBJECT)
  id: String
  type: IssueRelationType!
  issueId: String!
  relatedIssueId: String!

## CommentCreateInput (INPUT_OBJECT)
  id: String
  body: String
  bodyData: JSON
  issueId: String
  projectUpdateId: String
  initiativeUpdateId: String
  postId: String
  documentContentId: String
  projectId: String
  initiativeId: String
  parentId: String
  createAsUser: String
  displayIconUrl: String
  createdAt: DateTime
  doNotSubscribeToIssue: Boolean
  createOnSyncedSlackThread: Boolean
  quotedText: String
  subscriberIds: [String]

## PaginationOrderBy (ENUM)
Values: createdAt, updatedAt

## StringComparator (INPUT_OBJECT)
  eq: String
  neq: String
  in: [String]
  nin: [String]
  eqIgnoreCase: String
  neqIgnoreCase: String
  startsWith: String
  startsWithIgnoreCase: String
  notStartsWith: String
  endsWith: String
  notEndsWith: String
  contains: String
  containsIgnoreCase: String
  notContains: String
  notContainsIgnoreCase: String
  containsIgnoreCaseAndAccent: String

## IDComparator (INPUT_OBJECT)
  eq: ID
  neq: ID
  in: [ID]
  nin: [ID]

## NumberComparator (INPUT_OBJECT)
  eq: Float
  neq: Float
  in: [Float]
  nin: [Float]
  lt: Float
  lte: Float
  gt: Float
  gte: Float

## DateComparator (INPUT_OBJECT)
  eq: DateTimeOrDuration
  neq: DateTimeOrDuration
  in: [DateTimeOrDuration]
  nin: [DateTimeOrDuration]
  lt: DateTimeOrDuration
  lte: DateTimeOrDuration
  gt: DateTimeOrDuration
  gte: DateTimeOrDuration

## NullableDateComparator (INPUT_OBJECT)
  eq: DateTimeOrDuration
  neq: DateTimeOrDuration
  in: [DateTimeOrDuration]
  nin: [DateTimeOrDuration]
  null: Boolean
  lt: DateTimeOrDuration
  lte: DateTimeOrDuration
  gt: DateTimeOrDuration
  gte: DateTimeOrDuration

## WorkflowStateFilter (INPUT_OBJECT)
  id: IDComparator
  createdAt: DateComparator
  updatedAt: DateComparator
  name: StringComparator
  description: StringComparator
  position: NumberComparator
  type: StringComparator
  team: TeamFilter
  issues: IssueCollectionFilter
  and: [WorkflowStateFilter]
  or: [WorkflowStateFilter]

## IssueLabelFilter (INPUT_OBJECT)
  id: IDComparator
  createdAt: DateComparator
  updatedAt: DateComparator
  name: StringComparator
  isGroup: BooleanComparator
  creator: NullableUserFilter
  team: NullableTeamFilter
  parent: IssueLabelFilter
  and: [IssueLabelFilter]
  or: [IssueLabelFilter]

## IssueRelationType (ENUM)
Values: blocks, duplicate, related, similar
