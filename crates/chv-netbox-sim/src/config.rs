//! Simulator configuration.

/// Page size used when a list request carries no `limit` parameter —
/// the NetBox `PAGINATE_COUNT` analog (default 50). To be pinned by
/// the first qualification `--record` run.
pub const DEFAULT_PAGE_SIZE: usize = 50;

/// Upper bound for requested page sizes, and the page size returned
/// for `limit=0` — the NetBox `MAX_PAGE_SIZE` analog (default 1000).
pub const MAX_PAGE_SIZE: usize = 1000;

/// Configuration for one simulator instance.
///
/// The default (via [`Default`]) accepts **no** tokens, so every
/// authenticated request is rejected — tests and the bin should
/// construct it through [`NetboxSimConfig::new`] or
/// [`NetboxSimConfig::with_tokens`].
#[derive(Clone, Debug)]
pub struct NetboxSimConfig {
    /// Token(s) accepted in `Authorization: Token <t>` headers. An
    /// empty list rejects every request with 401.
    pub tokens: Vec<String>,
    /// Page size when a list request carries no `limit`.
    pub default_page_size: usize,
    /// Cap for requested page sizes; also the page size for
    /// `limit=0`.
    pub max_page_size: usize,
}

impl NetboxSimConfig {
    /// A config accepting exactly one token, with NetBox's default
    /// page sizes.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            tokens: vec![token.into()],
            default_page_size: DEFAULT_PAGE_SIZE,
            max_page_size: MAX_PAGE_SIZE,
        }
    }

    /// A config accepting several tokens, with NetBox's default page
    /// sizes.
    pub fn with_tokens<I, T>(tokens: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<String>,
    {
        Self {
            tokens: tokens.into_iter().map(Into::into).collect(),
            default_page_size: DEFAULT_PAGE_SIZE,
            max_page_size: MAX_PAGE_SIZE,
        }
    }

    /// Override the page sizes — mainly so tests can force multi-page
    /// pagination without seeding hundreds of objects.
    pub fn with_page_sizes(mut self, default_page_size: usize, max_page_size: usize) -> Self {
        self.default_page_size = default_page_size;
        self.max_page_size = max_page_size;
        self
    }
}

impl Default for NetboxSimConfig {
    fn default() -> Self {
        Self {
            tokens: Vec::new(),
            default_page_size: DEFAULT_PAGE_SIZE,
            max_page_size: MAX_PAGE_SIZE,
        }
    }
}
