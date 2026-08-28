export interface BlogUser {
  id: string
  email: string
  displayName: string
  avatar?: string
  bio?: string
  role?: string
  website?: string
  createdAt?: string
}

export interface BlogPost {
  id: string
  title: string
  slug?: string
  content: string
  excerpt?: string
  cover_image?: string
  is_public: boolean
  featured?: boolean
  category?: string
  tags?: string[]
  views?: number
  likes?: number
  author?: string
  created_at?: string
  updated_at?: string
  expand?: {
    author?: BlogUser
    category?: BlogCategory
  }
}

export interface BlogComment {
  id: string
  post: string
  author?: string
  author_name?: string
  author_avatar?: string
  author_email?: string
  content: string
  parent_id?: string
  created_at: string
  expand?: {
    author?: BlogUser
  }
}

export interface BlogCategory {
  id: string
  name: string
  slug: string
  description?: string
  icon?: string
  color?: string
  count?: number
}

export interface BlogTag {
  name: string
  count: number
  color?: string
}

export interface TOCItem {
  id: string
  text: string
  level: number
}

export interface PostFilter {
  category?: string
  tag?: string
  search?: string
  author?: string
  status?: 'all' | 'published' | 'draft'
  sortBy?: 'latest' | 'oldest' | 'views' | 'likes'
  page?: number
  perPage?: number
}

export interface SiteConfig {
  siteTitle: string
  siteDescription: string
  authorName: string
  authorAvatar: string
  authorBio: string
  postsPerPage: number
  githubUrl: string
}
