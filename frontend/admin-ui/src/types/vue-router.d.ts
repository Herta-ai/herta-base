import 'vue-router'

declare module 'vue-router' {
  interface RouteMeta {
    /**
     * Title of the route
     *
     * It can be used in document title
     */
    title: string
    /**
     * I18n key of the route
     *
     * It's used in i18n, if it is set, the title will be ignored
     */
    i18nKey?: I18n.I18nKey | null
    /** Whether to cache the route */
    keepAlive?: boolean | null
    /** Whether the route is do not need authorization */
    unAuth?: boolean | null
    /** if set query parameters, it will be automatically carried when entering the route */
    query?: { key: string, value: string }[] | null
  }
}
