/** The storage namespace */
declare namespace StorageType {
  interface Session {}

  interface Local {
    /** The token */
    token: string
    /** The theme color */
    themeColor: string
    /** The dark mode */
    darkMode: boolean
  }
}
