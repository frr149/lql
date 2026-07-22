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
          id
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

/// Borrar un comentario por su id.
pub const COMMENT_DELETE_MUTATION: &str = r#"
mutation DeleteComment($id: String!) {
  commentDelete(id: $id) {
    success
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
///
/// Nested page sizes are deliberately small: Linear rejects any query above a
/// ~10,000-point complexity budget, and nested connection page sizes multiply.
/// `projects(first: 5)` × `teams(first: 3)` keeps the worst case (first: 250)
/// well inside the budget. lql-managed epics have a single backing project, so
/// these caps never truncate in practice.
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
      projects(first: 5) {
        nodes {
          id
          name
          slugId
          teams(first: 3) {
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
///
/// Deliberately does NOT nest `issues` under each project: that extra
/// connection level multiplied page sizes past Linear's complexity budget.
/// `epic view` fetches the issues separately via `ISSUES_QUERY` filtered by
/// project id. `content` holds the long markdown body (`description` is the
/// short summary; older epics may still carry their body there).
pub const INITIATIVE_BY_REF_QUERY: &str = r#"
query InitiativeByRef($filter: InitiativeFilter) {
  initiatives(filter: $filter, first: 1) {
    nodes {
      id
      slugId
      name
      description
      content
      status
      targetDate
      createdAt
      url
      projects(first: 25) {
        nodes {
          id
          name
          slugId
          url
          targetDate
          teams(first: 5) {
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
      content
      status
      targetDate
      createdAt
      url
    }
  }
}
"#;

/// Borrar epic (rollback de un `epic create` parcialmente fallido)
pub const INITIATIVE_DELETE_MUTATION: &str = r#"
mutation DeleteInitiative($id: String!) {
  initiativeDelete(id: $id) {
    success
  }
}
"#;

/// Borrar project (rollback de un `epic create` parcialmente fallido)
pub const PROJECT_DELETE_MUTATION: &str = r#"
mutation DeleteProject($id: String!) {
  projectDelete(id: $id) {
    success
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

/// Actualizar epic (Linear initiative)
///
/// Field selection mirrors `INITIATIVE_BY_REF_QUERY` so the caller can reuse
/// the same formatter without a second read-back. As with create, the long
/// markdown body lives in `content`; `description` is the short summary.
pub const INITIATIVE_UPDATE_MUTATION: &str = r#"
mutation UpdateInitiative($id: String!, $input: InitiativeUpdateInput!) {
  initiativeUpdate(id: $id, input: $input) {
    success
    initiative {
      id
      slugId
      name
      description
      content
      status
      targetDate
      createdAt
      url
    }
  }
}
"#;

/// Actualizar project (backing project de un epic, o project genérico)
pub const PROJECT_UPDATE_MUTATION: &str = r#"
mutation UpdateProject($id: String!, $input: ProjectUpdateInput!) {
  projectUpdate(id: $id, input: $input) {
    success
    project {
      id
      slugId
      name
      description
      content
      url
      targetDate
      teams(first: 5) {
        nodes {
          key
        }
      }
    }
  }
}
"#;

/// Buscar project por slugId / UUID / nombre
pub const PROJECT_BY_REF_QUERY: &str = r#"
query ProjectByRef($filter: ProjectFilter) {
  projects(filter: $filter, first: 1) {
    nodes {
      id
      slugId
      name
      description
      content
      url
      targetDate
      teams(first: 5) {
        nodes {
          key
        }
      }
      initiatives(first: 5) {
        nodes {
          id
          slugId
          name
        }
      }
    }
  }
}
"#;

#[cfg(test)]
mod budget_tests {
    //! Network-free guards for the PR #12 epic regression (TOOL-128).
    //!
    //! Linear rejects any GraphQL query above a ~10,000-point complexity budget,
    //! and nested connection page sizes multiply. `epic` broke because its
    //! queries nested connections (notably an `issues` connection under every
    //! project) past that budget. These tests keep the epic queries inside it
    //! without touching the network — so the regression is guarded in CI, not
    //! only by the live `#[ignore]`d smoke tests.

    const LINEAR_COMPLEXITY_BUDGET: u64 = 10_000;

    /// Conservative proxy for query complexity: the product of every literal
    /// `first: N` page size. It over-counts sibling connections (treating them
    /// as if they multiplied), so it is stricter than Linear's real cost — the
    /// safe direction for a guard. `first: $var` (non-literal) is ignored.
    fn page_size_product(query: &str) -> u64 {
        let mut product: u64 = 1;
        let mut rest = query;
        while let Some(pos) = rest.find("first:") {
            rest = &rest[pos + "first:".len()..];
            let digits: String = rest
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(n) = digits.parse::<u64>() {
                product = product.saturating_mul(n.max(1));
            }
        }
        product
    }

    #[test]
    fn initiatives_query_within_budget() {
        let p = page_size_product(super::INITIATIVES_QUERY);
        assert!(
            p < LINEAR_COMPLEXITY_BUDGET,
            "INITIATIVES_QUERY page-size product {p} exceeds Linear's budget"
        );
    }

    #[test]
    fn initiative_by_ref_query_within_budget() {
        let p = page_size_product(super::INITIATIVE_BY_REF_QUERY);
        assert!(
            p < LINEAR_COMPLEXITY_BUDGET,
            "INITIATIVE_BY_REF_QUERY page-size product {p} exceeds Linear's budget"
        );
    }

    /// The PR #12 fix pulled the `issues` connection OUT of the by-ref query
    /// (it was nested under each project, multiplying page sizes). `epic view`
    /// now fetches issues with a separate flat query, so the by-ref query must
    /// never reintroduce a nested `issues` connection.
    #[test]
    fn initiative_by_ref_query_does_not_nest_issues() {
        assert!(
            !super::INITIATIVE_BY_REF_QUERY.contains("issues"),
            "INITIATIVE_BY_REF_QUERY must not nest an `issues` connection (PR #12 regression)"
        );
    }

    /// Anti-guard / positive control (P06): the proxy must flag a real
    /// budget-buster, or it proves nothing.
    #[test]
    fn page_size_product_detects_budget_buster() {
        let bad = "projects(first: 200) { nodes { issues(first: 200) { nodes { id } } } }";
        assert!(page_size_product(bad) >= LINEAR_COMPLEXITY_BUDGET);
    }
}
