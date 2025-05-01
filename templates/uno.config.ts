#if unocss
import {
  defineConfig,
  transformerDirectives,
  presetWind,
  presetWebFonts,
  presetIcons,
 } from 'unocss'
import { presetFluid } from 'unocss-preset-fluid'

export default defineConfig({
  cli: {
    entry: [
      {
        patterns: [
          '**/*.html', '*.html',
          #if astro
          '**/*.md', '*.md',
          '**/*.mdx', '*.mdx',
          '**/*.astro', '*.astro',
          #endif astro
        ],
        outFile: 'public/uno.css'
      }
    ], // CliEntryItem | CliEntryItem[]
  },
  transformers: [transformerDirectives()],
  shortcuts: [],
  theme: {
    colors: {
      'space-cadet': '#282D3F',
      cornflower: {
        light: '#97b6f0',
        DEFAULT: '#6E98E8',
      },
      sienna: {
        light: '#ecb1a2',
        mid: '#E28E78',
        DEFAULT: '#DC755C',
      },
    },
  },
  extendTheme: (theme) => {
    return {
      ...theme,
      breakpoints: {
        xs: '520px',
        ...theme.breakpoints
      }
    }
  },
  presets: [
    presetIcons(),
    presetWebFonts({
      provider: 'google',
      fonts: {
        sans: 'Fira Sans:100,200,300,400,500,600,700,800,900:italic',
        inter: 'Inter:100,200,300,400,500,600,700,800,900:italic',
        noto: 'Noto Serif:100,200,300,400,500,600,700,800,900:italic',
        mono: 'Fira Code:100,200,300,400,500,600,700,800,900:italic',
        serif: 'IBM Plex Serif:100,200,300,400,500,600,700,800,900:italic'
      },
    }),
    presetWind(),
    presetFluid({
      maxWidth: 1440,
      minWidth: 320,
      extendMaxWidth: null,
      extendMinWidth: null,
      remBase: 16,
      useRemByDefault: false,
      ranges: {
        // Got by doing {320px, 16px, 1.125}, {1440px, 18px, 1.25} on https://utopia.fyi
        '4xl': [32.44, 68.66],
        '3xl': [28.83, 54.93],
        '2xl': [25.63, 43.95],
        xl: [22.78, 35.16],
        lg: [20.25, 28.13],
        md: [18.00, 22.50],
        sm: [16.00, 18.00],
        xs: [14.22, 14.40],
        '2xs': [12.64, 11.52],
      },
      commentHelpers: false,
    }),
  ],
  // ...
})
#endif unocss
