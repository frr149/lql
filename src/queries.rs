/// Metadata query: teams, states, projects, labels — todo lo necesario para resolver nombres
pub const META_QUERY: &str = r#"
{
  teams {
    nodes {
      id
      key
      name
      states {
        nodes {
          id
          name
          type
        }
      }
      projects {
        nodes {
          id
          name
        }
      }
    }
  }
  issueLabels {
    nodes {
      id
      name
      team {
        key
      }
    }
  }
}
"#;

/// Listar issues con filtros
pub const ISSUES_QUERY: &str = r#"
query ListIssues($filter: IssueFilter, $first: Int, $orderBy: PaginationOrderBy) {
  issues(filter: $filter, first: $first, orderBy: $orderBy) {
    nodes {
      id
      identifier
      title
      priority
      state {
        name
        type
      }
      labels {
        nodes {
          name
        }
      }
      project {
        id
        name
      }
      team {
        key
      }
      createdAt
      dueDate
      url
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

/// Crear issue
pub const CREATE_MUTATION: &str = r#"
mutation CreateIssue($input: IssueCreateInput!) {
  issueCreate(input: $input) {
    success
    issue {
      id
      identifier
      title
      url
      state {
        name
        type
      }
      labels {
        nodes {
          name
        }
      }
    }
  }
}
"#;

/// Actualizar issue
pub const UPDATE_MUTATION: &str = r#"
mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {
  issueUpdate(id: $id, input: $input) {
    success
    issue {
      id
      identifier
      title
      state {
        name
        type
      }
      labels {
        nodes {
          name
        }
      }
    }
  }
}
"#;

/// Buscar issues por identifier (PROD-587 → UUID)
pub const ISSUE_BY_IDENTIFIER: &str = r#"
query IssueByIdentifier($filter: IssueFilter) {
  issues(filter: $filter, first: 1) {
    nodes {
      id
      identifier
      title
      priority
      description
      state {
        name
        type
      }
      labels {
        nodes {
          name
        }
      }
      project {
        name
      }
      team {
        key
      }
      createdAt
      startedAt
      completedAt
      updatedAt
      dueDate
      url
      relations {
        nodes {
          type
          relatedIssue {
            identifier
            title
          }
        }
      }
      comments {
        nodes {
          id
          body
          createdAt
          user {
            name
          }
        }
      }
    }
  }
}
"#;

/// Buscar issues por texto
pub const SEARCH_QUERY: &str = r#"
query SearchIssues($term: String!, $filter: IssueFilter, $first: Int) {
  searchIssues(term: $term, filter: $filter, first: $first) {
    nodes {
      id
      identifier
      title
      priority
      state {
        name
        type
      }
      labels {
        nodes {
          name
        }
      }
      project {
        name
      }
      team {
        key
      }
      createdAt
      dueDate
      url
    }
  }
}
"#;

/// Crear comentario
pub const COMMENT_MUTATION: &str = r#"
mutation CreateComment($input: CommentCreateInput!) {
  commentCreate(input: $input) {
    success
    comment {
      id
    }
  }
}
"#;

/// Crear relación
pub const RELATION_MUTATION: &str = r#"
mutation CreateRelation($input: IssueRelationCreateInput!) {
  issueRelationCreate(input: $input) {
    success
  }
}
"#;

/// Obtener relaciones de un issue (con IDs para poder borrarlas)
pub const ISSUE_RELATIONS_QUERY: &str = r#"
query IssueRelations($filter: IssueFilter) {
  issues(filter: $filter, first: 1) {
    nodes {
      identifier
      relations {
        nodes {
          id
          type
          relatedIssue {
            identifier
          }
        }
      }
      inverseRelations {
        nodes {
          id
          type
          issue {
            identifier
          }
        }
      }
    }
  }
}
"#;

/// Borrar relación
pub const RELATION_DELETE_MUTATION: &str = r#"
mutation DeleteRelation($id: String!) {
  issueRelationDelete(id: $id) {
    success
  }
}
"#;

/// Listar epics (Linear initiatives)
pub const INITIATIVES_QUERY: &str = r#"
query ListInitiatives($filter: InitiativeFilter, $first: Int, $orderBy: PaginationOrderBy) {
  initiatives(filter: $filter, first: $first, orderBy: $orderBy) {
    nodes {
      id
      slugId
      name
      description
      status
      targetDate
      createdAt
      url
      projects(first: 25) {
        nodes {
          id
          name
          slugId
          teams {
            nodes {
              key
            }
          }
        }
      }
    }
  }
}
"#;

/// Buscar epic por slugId / UUID
pub const INITIATIVE_BY_REF_QUERY: &str = r#"
query InitiativeByRef($filter: InitiativeFilter) {
  initiatives(filter: $filter, first: 1) {
    nodes {
      id
      slugId
      name
      description
      status
      targetDate
      createdAt
      url
      projects(first: 50) {
        nodes {
          id
          name
          slugId
          url
          targetDate
          teams {
            nodes {
              key
            }
          }
          issues(first: 250) {
            nodes {
              id
              identifier
              title
              priority
              state {
                name
                type
              }
              labels {
                nodes {
                  name
                }
              }
              project {
                id
                name
              }
              team {
                key
              }
              createdAt
              dueDate
              url
            }
          }
        }
      }
    }
  }
}
"#;

/// Crear epic
pub const INITIATIVE_CREATE_MUTATION: &str = r#"
mutation CreateInitiative($input: InitiativeCreateInput!) {
  initiativeCreate(input: $input) {
    success
    initiative {
      id
      slugId
      name
      description
      status
      targetDate
      createdAt
      url
    }
  }
}
"#;

/// Crear project de backing para un epic
pub const PROJECT_CREATE_MUTATION: &str = r#"
mutation CreateProject($input: ProjectCreateInput!) {
  projectCreate(input: $input) {
    success
    project {
      id
      name
      slugId
      url
      teams {
        nodes {
          key
        }
      }
    }
  }
}
"#;

/// Enlazar project a epic
pub const INITIATIVE_TO_PROJECT_CREATE_MUTATION: &str = r#"
mutation LinkInitiativeProject($input: InitiativeToProjectCreateInput!) {
  initiativeToProjectCreate(input: $input) {
    success
    initiativeToProject {
      id
    }
  }
}
"#;

/// Crear label
pub const LABEL_CREATE_MUTATION: &str = r#"
mutation CreateLabel($input: IssueLabelCreateInput!) {
  issueLabelCreate(input: $input) {
    success
    issueLabel {
      id
      name
      color
    }
  }
}
"#;

/// Borrar label
pub const LABEL_DELETE_MUTATION: &str = r#"
mutation DeleteLabel($id: String!) {
  issueLabelDelete(id: $id) {
    success
  }
}
"#;

/// Viewer (para doctor)
pub const VIEWER_QUERY: &str = r#"
{
  viewer {
    id
    name
    email
  }
}
"#;
