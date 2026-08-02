declare namespace Route {
  type RouteMap = {
    [K in keyof import('vue-router/auto-routes').RouteNamedMap]:
    import('vue-router/auto-routes').RouteNamedMap[K] extends import('vue-router').RouteRecordInfo<any, infer Path, any, any, any>
      ? Path
      : never
  }

  type RouteKey = keyof RouteMap

  type RoutePath = RouteMap[RouteKey]
}
