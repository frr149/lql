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

/// Ver issue con detalle
pub const VIEW_QUERY: &str = r#"
query ViewIssue($id: String!) {
  issue(id: $id) {
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
