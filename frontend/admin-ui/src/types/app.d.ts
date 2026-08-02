/** The global namespace for the app */
declare namespace App {
  /** Global namespace */
  namespace Global {
    type VNode = import('vue').VNode
    type RouteLocationNormalizedLoaded = import('vue-router').RouteLocationNormalizedLoaded
    type RouteKey = Route.RouteKey
    type RouteMap = Route.RouteMap
    type RoutePath = Route.RoutePath
    /** The router push options */
    interface RouterPushOptions {
      query?: Record<string, string>
      params?: Record<string, string>
    }
  }
  /** Theme namespace */
  namespace Theme {
    type ColorPaletteNumber = import('@/utils/color').ColorPaletteNumber

    /** NaiveUI theme overrides that can be specified in preset */
    type NaiveUIThemeOverride = import('naive-ui').GlobalThemeOverrides

    interface OtherColor {
      info: string
      success: string
      warning: string
      error: string
    }

    interface ThemeColor extends OtherColor {
      primary: string
    }

    type ThemeColorKey = keyof ThemeColor

    type ThemePaletteColor = {
      [key in ThemeColorKey | `${ThemeColorKey}-${ColorPaletteNumber}`]: string;
    }

    type BaseToken = Record<string, Record<string, string>>

    interface ThemeSettingTokenColor {
      /** the progress bar color, if not set, will use the primary color */
      'nprogress'?: string
      'container': string
      'layout': string
      'inverted': string
      'base-text': string
    }

    interface ThemeSettingTokenBoxShadow {
      header: string
      sider: string
      tab: string
    }

    interface ThemeSettingToken {
      colors: ThemeSettingTokenColor
      boxShadow: ThemeSettingTokenBoxShadow
    }

    type ThemeTokenColor = ThemePaletteColor & ThemeSettingTokenColor

    /** Theme token CSS variables */
    interface ThemeTokenCSSVars {
      colors: ThemeTokenColor & { [key: string]: string }
      boxShadow: ThemeSettingTokenBoxShadow & { [key: string]: string }
    }

    /** Theme setting */
    interface ThemeSetting {
      /** Theme scheme */
      themeScheme: UnionKey.ThemeScheme
      /** grayscale mode */
      grayscale: boolean
      /** colour weakness mode */
      colorWeakness: boolean
      /** Whether to recommend color */
      recommendColor: boolean
      /** Theme color */
      themeColor: string
      /** Theme radius */
      themeRadius: number
      /** Other color */
      otherColor: OtherColor
      /** Whether info color is followed by the primary color */
      isInfoFollowPrimary: boolean
      /** Page */
      page: {
        /** Whether to show the page transition */
        animate: boolean
        /** Page animate mode */
        animateMode: UnionKey.ThemePageAnimateMode
      }
      /** Watermark */
      watermark: {
        /** Whether to show the watermark */
        visible: boolean
        /** Watermark text */
        text: string
        /** Whether to use user name as watermark text */
        enableUserName: boolean
        /** Whether to use current time as watermark text */
        enableTime: boolean
        /** Time format for watermark text */
        timeFormat: string
      }
      /** define some theme settings tokens, will transform to css variables */
      tokens: {
        light: ThemeSettingToken
        dark?: {
          [K in keyof ThemeSettingToken]?: Partial<ThemeSettingToken[K]>;
        }
      }
    }
  }
}
