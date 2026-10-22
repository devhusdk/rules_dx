//! Pinned pnpm lockfiles for the JavaScript framework starters.

pub const ASTRO_LOCK: &str = r#"lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    devDependencies:
      '@astrojs/compiler':
        specifier: 4.0.0
        version: 4.0.0

packages:

  '@astrojs/compiler@4.0.0':
    resolution: {integrity: sha512-eouss7G8ygdZqHuke033VMcVw5HTZUu+PXd/h06DGDUg/jt5btPYPqh66ENWw/mU78rBrf/oeC4oqoBwMtDMNA==}

snapshots:

  '@astrojs/compiler@4.0.0': {}
"#;

pub const SVELTE_LOCK: &str = r#"lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    devDependencies:
      svelte:
        specifier: 5.57.0
        version: 5.57.0

packages:

  '@jridgewell/gen-mapping@0.3.13':
    resolution: {integrity: sha512-2kkt/7niJ6MgEPxF0bYdQ6etZaA+fQvDcLKckhy1yIQOzaoKjBBjSj63/aLVjYE3qhRt5dvM+uUyfCg6UKCBbA==}

  '@jridgewell/remapping@2.3.5':
    resolution: {integrity: sha512-LI9u/+laYG4Ds1TDKSJW2YPrIlcVYOwi2fUC6xB43lueCjgxV4lffOCZCtYFiH6TNOX+tQKXx97T4IKHbhyHEQ==}

  '@jridgewell/resolve-uri@3.1.2':
    resolution: {integrity: sha512-bRISgCIjP20/tbWSPWMEi54QVPRZExkuD9lJL+UIxUKtwVJA8wW1Trb1jMs1RFXo1CBTNZ/5hpC9QvmKWdopKw==}
    engines: {node: '>=6.0.0'}

  '@jridgewell/sourcemap-codec@1.6.0':
    resolution: {integrity: sha512-T7jf+5zgsZHwNJ4lvQ7/aezbyk0nNX+zJVWpmHA7VYsEx7a7qr5Rg5IbtJFqkgze5Y2sruq1RUY8Q837Od7iFw==}

  '@jridgewell/trace-mapping@0.3.31':
    resolution: {integrity: sha512-zzNR+SdQSDJzc8joaeP8QQoCQr8NuYx2dIIytl1QeBEZHJ9uW6hebsrYgbz8hJwUQao3TWCMtmfV8Nu1twOLAw==}

  '@sveltejs/acorn-typescript@1.0.13':
    resolution: {integrity: sha512-wgKggnhZVL9Bfx1OaKKTrYY9BFRk6C8UAkQNUcIv1+llzYrIqy+RZm5HPKzn0NpEBvTVhTqB4kQyllZywsRBRQ==}
    peerDependencies:
      acorn: ^8.9.0

  '@types/estree@1.0.9':
    resolution: {integrity: sha512-GhdPgy1el4/ImP05X05Uw4cw2/M93BCUmnEvWZNStlCzEKME4Fkk+YpoA5OiHNQmoS7Cafb8Xa3Pya8m1Qrzeg==}

  acorn@8.19.0:
    resolution: {integrity: sha512-oJlA3XiRm7Cyk6qFD2Jn8ak9B7jSy0qy00ADO3+8dpT0LSjFihQYv4C02LFCSYJV3Q37xYwwRy5m+IpIiUzqWw==}
    engines: {node: '>=0.4.0'}
    hasBin: true

  aria-query@5.3.1:
    resolution: {integrity: sha512-Z/ZeOgVl7bcSYZ/u/rh0fOpvEpq//LZmdbkXyc7syVzjPAhfOa9ebsdTSjEBDU4vs5nC98Kfduj1uFo0qyET3g==}
    engines: {node: '>= 0.4'}

  axobject-query@4.1.0:
    resolution: {integrity: sha512-qIj0G9wZbMGNLjLmg1PT6v2mE9AH2zlnADJD/2tC6E00hgmhUOfEB6greHPAfLRSufHqROIUTkw6E+M3lH0PTQ==}
    engines: {node: '>= 0.4'}

  clsx@2.1.1:
    resolution: {integrity: sha512-eYm0QWBtUrBWZWG0d386OGAw16Z995PiOVo2B7bjWSbHedGl5e0ZWaq65kOGgUSNesEIDkB9ISbTg/JK9dhCZA==}
    engines: {node: '>=6'}

  devalue@5.9.4:
    resolution: {integrity: sha512-sPAT4pztbu6586/hrhOnMKS17IJrvg12mXiSPSS3W5qDeN2RGgvZ0diZCm31dBbnevfVmujNO3IM2wrS4Y2Rhg==}

  esm-env@1.2.2:
    resolution: {integrity: sha512-Epxrv+Nr/CaL4ZcFGPJIYLWFom+YeV1DqMLHJoEd9SYRxNbaFruBwfEX/kkHUJf55j2+TUbmDcmuilbP1TmXHA==}

  esrap@2.4.0:
    resolution: {integrity: sha512-CQPcneEHw/czj32uwQC52nYnax8H4SXt25FCM+LIwGZ49cMIxpOj3bP8wmzcI+Z0HIy5KkClaPhRLfCySzwHPA==}
    peerDependencies:
      '@typescript-eslint/types': ^8.2.0
    peerDependenciesMeta:
      '@typescript-eslint/types':
        optional: true

  is-reference@3.0.3:
    resolution: {integrity: sha512-ixkJoqQvAP88E6wLydLGGqCJsrFUnqoH6HnaczB8XmDH1oaWU+xxdptvikTgaEhtZ53Ky6YXiBuUI2WXLMCwjw==}

  locate-character@3.0.0:
    resolution: {integrity: sha512-SW13ws7BjaeJ6p7Q6CO2nchbYEc3X3J6WrmTTDto7yMPqVSZTUyY5Tjbid+Ab8gLnATtygYtiDIJGQRRn2ZOiA==}

  magic-string@0.30.21:
    resolution: {integrity: sha512-vd2F4YUyEXKGcLHoq+TEyCjxueSeHnFxyyjNp80yg0XV4vUhnDer/lvvlqM/arB5bXQN5K2/3oinyCRyx8T2CQ==}

  svelte@5.57.0:
    resolution: {integrity: sha512-NdbDn7fl4be1ViUG0oq/lvG6OZy3oENolV2ONjiqqsfVoeAfzaQAKUcEX3MrQod/Bebv1PgwET9rfXhgn9s4Kg==}
    engines: {node: '>=18'}

  zimmerframe@1.1.5:
    resolution: {integrity: sha512-msJxIvYDYcoNL+PJsu+7qmpDWsYmAxTY+2TNYXXF0hzBzBk0BMecOqDOG/EckUoKCuKwObfbugIl8QpqHDXeFA==}

snapshots:

  '@jridgewell/gen-mapping@0.3.13':
    dependencies:
      '@jridgewell/sourcemap-codec': 1.6.0
      '@jridgewell/trace-mapping': 0.3.31

  '@jridgewell/remapping@2.3.5':
    dependencies:
      '@jridgewell/gen-mapping': 0.3.13
      '@jridgewell/trace-mapping': 0.3.31

  '@jridgewell/resolve-uri@3.1.2': {}

  '@jridgewell/sourcemap-codec@1.6.0': {}

  '@jridgewell/trace-mapping@0.3.31':
    dependencies:
      '@jridgewell/resolve-uri': 3.1.2
      '@jridgewell/sourcemap-codec': 1.6.0

  '@sveltejs/acorn-typescript@1.0.13(acorn@8.19.0)':
    dependencies:
      acorn: 8.19.0

  '@types/estree@1.0.9': {}

  acorn@8.19.0: {}

  aria-query@5.3.1: {}

  axobject-query@4.1.0: {}

  clsx@2.1.1: {}

  devalue@5.9.4: {}

  esm-env@1.2.2: {}

  esrap@2.4.0:
    dependencies:
      '@jridgewell/sourcemap-codec': 1.6.0

  is-reference@3.0.3:
    dependencies:
      '@types/estree': 1.0.9

  locate-character@3.0.0: {}

  magic-string@0.30.21:
    dependencies:
      '@jridgewell/sourcemap-codec': 1.6.0

  svelte@5.57.0:
    dependencies:
      '@jridgewell/remapping': 2.3.5
      '@jridgewell/sourcemap-codec': 1.6.0
      '@sveltejs/acorn-typescript': 1.0.13(acorn@8.19.0)
      '@types/estree': 1.0.9
      acorn: 8.19.0
      aria-query: 5.3.1
      axobject-query: 4.1.0
      clsx: 2.1.1
      devalue: 5.9.4
      esm-env: 1.2.2
      esrap: 2.4.0
      is-reference: 3.0.3
      locate-character: 3.0.0
      magic-string: 0.30.21
      zimmerframe: 1.1.5
    transitivePeerDependencies:
      - '@typescript-eslint/types'

  zimmerframe@1.1.5: {}
"#;

pub const VUE_LOCK: &str = r#"lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    devDependencies:
      '@vue/compiler-sfc':
        specifier: 3.5.42
        version: 3.5.42

packages:

  '@babel/helper-string-parser@7.29.7':
    resolution: {integrity: sha512-Pb5ijPrZ89GDH8223L4UP8i6QApWxs04RbPQJTeWDV0/keR2E36MeKnyr6LYmUUvqRRI+Iv87SuF1W6ErINzYw==}
    engines: {node: '>=6.9.0'}

  '@babel/helper-validator-identifier@7.29.7':
    resolution: {integrity: sha512-qehxGkRj55h/ff8EMaJ+cYhyaKlHIxqYDn682wQD7RNp9UujOQsHog2uS0r2vzr4pW+sXf90NeeayjcNaX3fFg==}
    engines: {node: '>=6.9.0'}

  '@babel/parser@7.29.9':
    resolution: {integrity: sha512-CjXrNHTnvqBVqHgdBysY3vk2T8tpJHb5/RMeHJBTyVa9xgugCB0CJTx/3oO8RV2QRQP391RWpB7D6hLjm8V9uA==}
    engines: {node: '>=6.0.0'}
    hasBin: true

  '@babel/types@7.29.8':
    resolution: {integrity: sha512-Vj1jF3cPfxg7OAfoI7QnVKLoILlm2JF9pnVHrX8qx7AHMiYWT+NDAA7jChlNgRS4WTLc/fD1lXLmPixluj+3Gg==}
    engines: {node: '>=6.9.0'}

  '@jridgewell/sourcemap-codec@1.6.0':
    resolution: {integrity: sha512-T7jf+5zgsZHwNJ4lvQ7/aezbyk0nNX+zJVWpmHA7VYsEx7a7qr5Rg5IbtJFqkgze5Y2sruq1RUY8Q837Od7iFw==}

  '@vue/compiler-core@3.5.42':
    resolution: {integrity: sha512-2Ye1ilMtKXxl8qZUrQ5j0CdgenFp/HFQmta6rfRyfEsTG69L6Wk+tWuNoHYHMx9E8tF2Slvdg1FuwDvAXdy1LQ==}

  '@vue/compiler-dom@3.5.42':
    resolution: {integrity: sha512-qbhQZEFmycr+ni/qyuccS4sucNN7VAbDfbkvNxWOX2VfgFm90MNs3/UhRNKoPMEIVn0F8gdlYjLPvqxHwHeQOA==}

  '@vue/compiler-sfc@3.5.42':
    resolution: {integrity: sha512-fkCAFB4okcAANGMThboWnScp/gzWjU0ZSkVnjTIiplmMDq2uq0tIB3j+xVu4rhv5rvOgBySCysudmbMd6xRRqw==}

  '@vue/compiler-ssr@3.5.42':
    resolution: {integrity: sha512-xmLk3wLkbizPAiLyomjgFFosf2ys9b5Ghb+oh/k2tnvipNz8OFrQOiTcWCzyK7MpBp9KkyGtfvgfLUivbmuGYA==}

  '@vue/shared@3.5.42':
    resolution: {integrity: sha512-2rPxex1jQf4jvl9MOHl6YaXCPcrNqz/FstMOEh3QWY+/OME9nQTvl9WYeCwhW7AFjaR0SnngZGlp/wkR6rkI6g==}

  entities@7.0.1:
    resolution: {integrity: sha512-TWrgLOFUQTH994YUyl1yT4uyavY5nNB5muff+RtWaqNVCAK408b5ZnnbNAUEWLTCpum9w6arT70i1XdQ4UeOPA==}
    engines: {node: '>=0.12'}

  estree-walker@2.0.2:
    resolution: {integrity: sha512-Rfkk/Mp/DL7JVje3u18FxFujQlTNR2q6QfMSMB7AvCBx91NGj/ba3kCfza0f6dVDbw7YlRf/nDrn7pQrCCyQ/w==}

  magic-string@0.30.21:
    resolution: {integrity: sha512-vd2F4YUyEXKGcLHoq+TEyCjxueSeHnFxyyjNp80yg0XV4vUhnDer/lvvlqM/arB5bXQN5K2/3oinyCRyx8T2CQ==}

  nanoid@3.3.20:
    resolution: {integrity: sha512-uKdg2G3GNCKQn9byYOpxbGqrT2fGO5KRt5J/8b3pok8rT6qxGWF6hxMyJiEYtAf+FVyYuD9hRaDqX5uPFYJ4ZQ==}
    engines: {node: ^10 || ^12 || ^13.7 || ^14 || >=15.0.1}
    hasBin: true

  picocolors@1.1.1:
    resolution: {integrity: sha512-xceH2snhtb5M9liqDsmEw56le376mTZkEX/jEb/RxNFyegNul7eNslCXP9FDj/Lcu0X8KEyMceP2ntpaHrDEVA==}

  postcss@8.5.31:
    resolution: {integrity: sha512-YUbWYCgErwnMAVkjUnYa+KlMBzGpHwiIt/DE7f0G33AuX2LH9UcTk6KgWxd3zRU+pN3u/C+Xl3EnMLM4Hea7Sg==}
    engines: {node: ^10 || ^12 || >=14}

  source-map-js@1.2.2:
    resolution: {integrity: sha512-KGj/8Y43x35aZVDtt+J4mK1hoLGHULMYfSkODJNQjNDC3oW1PqPoxMwo0pLUsWM/UEGzON/NxeHywEfNXNP3Vw==}
    engines: {node: '>=0.10.0'}

snapshots:

  '@babel/helper-string-parser@7.29.7': {}

  '@babel/helper-validator-identifier@7.29.7': {}

  '@babel/parser@7.29.9':
    dependencies:
      '@babel/types': 7.29.8

  '@babel/types@7.29.8':
    dependencies:
      '@babel/helper-string-parser': 7.29.7
      '@babel/helper-validator-identifier': 7.29.7

  '@jridgewell/sourcemap-codec@1.6.0': {}

  '@vue/compiler-core@3.5.42':
    dependencies:
      '@babel/parser': 7.29.9
      '@vue/shared': 3.5.42
      entities: 7.0.1
      estree-walker: 2.0.2
      source-map-js: 1.2.2

  '@vue/compiler-dom@3.5.42':
    dependencies:
      '@vue/compiler-core': 3.5.42
      '@vue/shared': 3.5.42

  '@vue/compiler-sfc@3.5.42':
    dependencies:
      '@babel/parser': 7.29.9
      '@vue/compiler-core': 3.5.42
      '@vue/compiler-dom': 3.5.42
      '@vue/compiler-ssr': 3.5.42
      '@vue/shared': 3.5.42
      estree-walker: 2.0.2
      magic-string: 0.30.21
      postcss: 8.5.31
      source-map-js: 1.2.2

  '@vue/compiler-ssr@3.5.42':
    dependencies:
      '@vue/compiler-dom': 3.5.42
      '@vue/shared': 3.5.42

  '@vue/shared@3.5.42': {}

  entities@7.0.1: {}

  estree-walker@2.0.2: {}

  magic-string@0.30.21:
    dependencies:
      '@jridgewell/sourcemap-codec': 1.6.0

  nanoid@3.3.20: {}

  picocolors@1.1.1: {}

  postcss@8.5.31:
    dependencies:
      nanoid: 3.3.20
      picocolors: 1.1.1
      source-map-js: 1.2.2

  source-map-js@1.2.2: {}
"#;

pub const MDX_LOCK: &str = r#"lockfileVersion: '9.0'

settings:
  autoInstallPeers: true
  excludeLinksFromLockfile: false

importers:

  .:
    devDependencies:
      '@mdx-js/mdx':
        specifier: 3.1.1
        version: 3.1.1

packages:

  '@mdx-js/mdx@3.1.1':
    resolution: {integrity: sha512-f6ZO2ifpwAQIpzGWaBQT2TXxPv6z3RBzQKpVftEWN78Vl/YweF1uwussDx8ECAXVtr3Rs89fKyG9YlzUs9DyGQ==}

  '@types/debug@4.1.13':
    resolution: {integrity: sha512-KSVgmQmzMwPlmtljOomayoR89W4FynCAi3E8PPs7vmDVPe84hT+vGPKkJfThkmXs0x0jAaa9U8uW8bbfyS2fWw==}

  '@types/estree-jsx@1.0.5':
    resolution: {integrity: sha512-52CcUVNFyfb1A2ALocQw/Dd1BQFNmSdkuC3BkZ6iqhdMfQz7JWOFRuJFloOzjk+6WijU56m9oKXFAXc7o3Towg==}

  '@types/estree@1.0.9':
    resolution: {integrity: sha512-GhdPgy1el4/ImP05X05Uw4cw2/M93BCUmnEvWZNStlCzEKME4Fkk+YpoA5OiHNQmoS7Cafb8Xa3Pya8m1Qrzeg==}

  '@types/hast@3.0.5':
    resolution: {integrity: sha512-rp/ezSWaD1m44dPKICGhiskI13nVr7qTloFwDa/IYkhhf5nzwP+zIQcIJh3WIFSBOy/H1PzB40jPjMDksN4F+g==}

  '@types/mdast@4.0.4':
    resolution: {integrity: sha512-kGaNbPh1k7AFzgpud/gMdvIm5xuECykRR+JnWKQno9TAXVa6WIVCGTPvYGekIDL4uwCZQSYbUxNBSb1aUo79oA==}

  '@types/mdx@2.0.14':
    resolution: {integrity: sha512-T48PeuJtvLosNTPVhfnIp3i/n3a4g4Bad7YCq5k64D4u7NwDrAotikQ+5+sjtUvBmxCMlbo3dVL+C2dP0rWHzg==}

  '@types/ms@2.1.0':
    resolution: {integrity: sha512-GsCCIZDE/p3i96vtEqx+7dBUGXrc7zeSK3wwPHIaRThS+9OhWIXRqzs4d6k1SVU8g91DrNRWxWUGhp5KXQb2VA==}

  '@types/unist@2.0.11':
    resolution: {integrity: sha512-CmBKiL6NNo/OqgmMn95Fk9Whlp2mtvIv+KNpQKN2F4SjvrEesubTRWGYSg+BnWZOnlCaSTU1sMpsBOzgbYhnsA==}

  '@types/unist@3.0.3':
    resolution: {integrity: sha512-ko/gIFJRv177XgZsZcBwnqJN5x/Gien8qNOn0D5bQU/zAzVf9Zt3BlcUiLqhV9y4ARk0GbT3tnUiPNgnTXzc/Q==}

  '@ungap/structured-clone@1.4.0':
    resolution: {integrity: sha512-1mEZtMKPM09vDmQt5y7YvmN2+DFTP7Tg0EWXdic8/C6VRnpb33e4ghisCIE3WZjsE2N8mf+QV1Zqh7ZFYLWInQ==}

  acorn-jsx@5.3.2:
    resolution: {integrity: sha512-rq9s+JNhf0IChjtDXxllJ7g41oZk5SlXtp0LHwyA5cejwn7vKmKp4pPri6YEePv2PU65sAsegbXtIinmDFDXgQ==}
    peerDependencies:
      acorn: ^6.0.0 || ^7.0.0 || ^8.0.0

  acorn@8.19.0:
    resolution: {integrity: sha512-oJlA3XiRm7Cyk6qFD2Jn8ak9B7jSy0qy00ADO3+8dpT0LSjFihQYv4C02LFCSYJV3Q37xYwwRy5m+IpIiUzqWw==}
    engines: {node: '>=0.4.0'}
    hasBin: true

  astring@1.9.0:
    resolution: {integrity: sha512-LElXdjswlqjWrPpJFg1Fx4wpkOCxj1TDHlSV4PlaRxHGWko024xICaa97ZkMfs6DRKlCguiAI+rbXv5GWwXIkg==}
    hasBin: true

  bail@2.0.2:
    resolution: {integrity: sha512-0xO6mYd7JB2YesxDKplafRpsiOzPt9V02ddPCLbY1xYGPOX24NTyN50qnUxgCPcSoYMhKpAuBTjQoRZCAkUDRw==}

  ccount@2.0.1:
    resolution: {integrity: sha512-eyrF0jiFpY+3drT6383f1qhkbGsLSifNAjA61IUjZjmLCWjItY6LB9ft9YhoDgwfmclB2zhu51Lc7+95b8NRAg==}

  character-entities-html4@2.1.0:
    resolution: {integrity: sha512-1v7fgQRj6hnSwFpq1Eu0ynr/CDEw0rXo2B61qXrLNdHZmPKgb7fqS1a2JwF0rISo9q77jDI8VMEHoApn8qDoZA==}

  character-entities-legacy@3.0.0:
    resolution: {integrity: sha512-RpPp0asT/6ufRm//AJVwpViZbGM/MkjQFxJccQRHmISF/22NBtsHqAWmL+/pmkPWoIUJdWyeVleTl1wydHATVQ==}

  character-entities@2.0.2:
    resolution: {integrity: sha512-shx7oQ0Awen/BRIdkjkvz54PnEEI/EjwXDSIZp86/KKdbafHh1Df/RYGBhn4hbe2+uKC9FnT5UCEdyPz3ai9hQ==}

  character-reference-invalid@2.0.1:
    resolution: {integrity: sha512-iBZ4F4wRbyORVsu0jPV7gXkOsGYjGHPmAyv+HiHG8gi5PtC9KI2j1+v8/tlibRvjoWX027ypmG/n0HtO5t7unw==}

  collapse-white-space@2.1.0:
    resolution: {integrity: sha512-loKTxY1zCOuG4j9f6EPnuyyYkf58RnhhWTvRoZEokgB+WbdXehfjFviyOVYkqzEWz1Q5kRiZdBYS5SwxbQYwzw==}

  comma-separated-tokens@2.0.3:
    resolution: {integrity: sha512-Fu4hJdvzeylCfQPp9SGWidpzrMs7tTrlu6Vb8XGaRGck8QSNZJJp538Wrb60Lax4fPwR64ViY468OIUTbRlGZg==}

  debug@4.4.3:
    resolution: {integrity: sha512-RGwwWnwQvkVfavKVt22FGLw+xYSdzARwm0ru6DhTVA3umU5hZc28V3kO4stgYryrTlLpuvgI9GiijltAjNbcqA==}
    engines: {node: '>=6.0'}
    peerDependencies:
      supports-color: '*'
    peerDependenciesMeta:
      supports-color:
        optional: true

  decode-named-character-reference@1.3.0:
    resolution: {integrity: sha512-GtpQYB283KrPp6nRw50q3U9/VfOutZOe103qlN7BPP6Ad27xYnOIWv4lPzo8HCAL+mMZofJ9KEy30fq6MfaK6Q==}

  dequal@2.0.3:
    resolution: {integrity: sha512-0je+qPKHEMohvfRTCEo3CrPG6cAzAYgmzKyxRiYSSDkS6eGJdyVJm7WaYA5ECaAD9wLB2T4EEeymA5aFVcYXCA==}
    engines: {node: '>=6'}

  devlop@1.1.0:
    resolution: {integrity: sha512-RWmIqhcFf1lRYBvNmr7qTNuyCt/7/ns2jbpp1+PalgE/rDQcBT0fioSMUpJ93irlUhC5hrg4cYqe6U+0ImW0rA==}

  esast-util-from-estree@2.0.0:
    resolution: {integrity: sha512-4CyanoAudUSBAn5K13H4JhsMH6L9ZP7XbLVe/dKybkxMO7eDyLsT8UHl9TRNrU2Gr9nz+FovfSIjuXWJ81uVwQ==}

  esast-util-from-js@2.0.1:
    resolution: {integrity: sha512-8Ja+rNJ0Lt56Pcf3TAmpBZjmx8ZcK5Ts4cAzIOjsjevg9oSXJnl6SUQ2EevU8tv3h6ZLWmoKL5H4fgWvdvfETw==}

  estree-util-attach-comments@3.0.0:
    resolution: {integrity: sha512-cKUwm/HUcTDsYh/9FgnuFqpfquUbwIqwKM26BVCGDPVgvaCl/nDCCjUfiLlx6lsEZ3Z4RFxNbOQ60pkaEwFxGw==}

  estree-util-build-jsx@3.0.1:
    resolution: {integrity: sha512-8U5eiL6BTrPxp/CHbs2yMgP8ftMhR5ww1eIKoWRMlqvltHF8fZn5LRDvTKuxD3DUn+shRbLGqXemcP51oFCsGQ==}

  estree-util-is-identifier-name@3.0.0:
    resolution: {integrity: sha512-hFtqIDZTIUZ9BXLb8y4pYGyk6+wekIivNVTcmvk8NoOh+VeRn5y6cEHzbURrWbfp1fIqdVipilzj+lfaadNZmg==}

  estree-util-scope@1.0.1:
    resolution: {integrity: sha512-B0np3dcdxqILX5e9nEi5/Fr4K7gL4oYFVPV1zRa2e9wRCbQoZZNWOZFYyoInvXUPJXBXjss+QXlWLJChDEHDkA==}

  estree-util-to-js@2.0.0:
    resolution: {integrity: sha512-WDF+xj5rRWmD5tj6bIqRi6CkLIXbbNQUcxQHzGysQzvHmdYG2G7p/Tf0J0gpxGgkeMZNTIjT/AoSvC9Xehcgdg==}

  estree-util-visit@2.0.0:
    resolution: {integrity: sha512-m5KgiH85xAhhW8Wta0vShLcUvOsh3LLPI2YVwcbio1l7E09NTLL1EyMZFM1OyWowoH0skScNbhOPl4kcBgzTww==}

  estree-walker@3.0.3:
    resolution: {integrity: sha512-7RUKfXgSMMkzt6ZuXmqapOurLGPPfgj6l9uRZ7lRGolvk0y2yocc35LdcxKC5PQZdn2DMqioAQ2NoWcrTKmm6g==}

  extend@3.0.2:
    resolution: {integrity: sha512-fjquC59cD7CyW6urNXK0FBufkZcoiGG80wTuPujX590cB5Ttln20E2UB4S/WARVqhXffZl2LNgS+gQdPIIim/g==}

  hast-util-to-estree@3.1.3:
    resolution: {integrity: sha512-48+B/rJWAp0jamNbAAf9M7Uf//UVqAoMmgXhBdxTDJLGKY+LRnZ99qcG+Qjl5HfMpYNzS5v4EAwVEF34LeAj7w==}

  hast-util-to-jsx-runtime@2.3.6:
    resolution: {integrity: sha512-zl6s8LwNyo1P9uw+XJGvZtdFF1GdAkOg8ujOw+4Pyb76874fLps4ueHXDhXWdk6YHQ6OgUtinliG7RsYvCbbBg==}

  hast-util-whitespace@3.0.0:
    resolution: {integrity: sha512-88JUN06ipLwsnv+dVn+OIYOvAuvBMy/Qoi6O7mQHxdPXpjy+Cd6xRkWwux7DKO+4sYILtLBRIKgsdpS2gQc7qw==}

  inline-style-parser@0.2.7:
    resolution: {integrity: sha512-Nb2ctOyNR8DqQoR0OwRG95uNWIC0C1lCgf5Naz5H6Ji72KZ8OcFZLz2P5sNgwlyoJ8Yif11oMuYs5pBQa86csA==}

  is-alphabetical@2.0.1:
    resolution: {integrity: sha512-FWyyY60MeTNyeSRpkM2Iry0G9hpr7/9kD40mD/cGQEuilcZYS4okz8SN2Q6rLCJ8gbCt6fN+rC+6tMGS99LaxQ==}

  is-alphanumerical@2.0.1:
    resolution: {integrity: sha512-hmbYhX/9MUMF5uh7tOXyK/n0ZvWpad5caBA17GsC6vyuCqaWliRG5K1qS9inmUhEMaOBIW7/whAnSwveW/LtZw==}

  is-decimal@2.0.1:
    resolution: {integrity: sha512-AAB9hiomQs5DXWcRB1rqsxGUstbRroFOPPVAomNk/3XHR5JyEZChOyTWe2oayKnsSsr/kcGqF+z6yuH6HHpN0A==}

  is-hexadecimal@2.0.1:
    resolution: {integrity: sha512-DgZQp241c8oO6cA1SbTEWiXeoxV42vlcJxgH+B3hi1AiqqKruZR3ZGF8In3fj4+/y/7rHvlOZLZtgJ/4ttYGZg==}

  is-plain-obj@4.1.0:
    resolution: {integrity: sha512-+Pgi+vMuUNkJyExiMBt5IlFoMyKnr5zhJ4Uspz58WOhBF5QoIZkFyNHIbBAtHwzVAgk5RtndVNsDRN61/mmDqg==}
    engines: {node: '>=12'}

  longest-streak@3.1.0:
    resolution: {integrity: sha512-9Ri+o0JYgehTaVBBDoMqIl8GXtbWg711O3srftcHhZ0dqnETqLaoIK0x17fUw9rFSlK/0NlsKe0Ahhyl5pXE2g==}

  markdown-extensions@2.0.0:
    resolution: {integrity: sha512-o5vL7aDWatOTX8LzaS1WMoaoxIiLRQJuIKKe2wAw6IeULDHaqbiqiggmx+pKvZDb1Sj+pE46Sn1T7lCqfFtg1Q==}
    engines: {node: '>=16'}

  mdast-util-from-markdown@2.1.0:
    resolution: {integrity: sha512-KRLgmPaP2H1ul2bpoJkOS+9pAcDAAj9Nu1mqLnh8NLeJG5tlvj42TH80PIGBpROWlRXLk0lDrWNuZCFy/tsB4A==}

  mdast-util-mdx-expression@2.0.1:
    resolution: {integrity: sha512-J6f+9hUp+ldTZqKRSg7Vw5V6MqjATc+3E4gf3CFNcuZNWD8XdyI6zQ8GqH7f8169MM6P7hMBRDVGnn7oHB9kXQ==}

  mdast-util-mdx-jsx@3.2.0:
    resolution: {integrity: sha512-lj/z8v0r6ZtsN/cGNNtemmmfoLAFZnjMbNyLzBafjzikOM+glrjNHPlf6lQDOTccj9n5b0PPihEBbhneMyGs1Q==}

  mdast-util-mdx@3.0.0:
    resolution: {integrity: sha512-JfbYLAW7XnYTTbUsmpu0kdBUVe+yKVJZBItEjwyYJiDJuZ9w4eeaqks4HQO+R7objWgS2ymV60GYpI14Ug554w==}

  mdast-util-mdxjs-esm@2.0.1:
    resolution: {integrity: sha512-EcmOpxsZ96CvlP03NghtH1EsLtr0n9Tm4lPUJUBccV9RwUOneqSycg19n5HGzCf+10LozMRSObtVr3ee1WoHtg==}

  mdast-util-phrasing@4.1.0:
    resolution: {integrity: sha512-TqICwyvJJpBwvGAMZjj4J2n0X8QWp21b9l0o7eXyVJ25YNWYbJDVIyD1bZXE6WtV6RmKJVYmQAKWa0zWOABz2w==}

  mdast-util-to-hast@13.2.1:
    resolution: {integrity: sha512-cctsq2wp5vTsLIcaymblUriiTcZd0CwWtCbLvrOzYCDZoWyMNV8sZ7krj09FSnsiJi3WVsHLM4k6Dq/yaPyCXA==}

  mdast-util-to-markdown@2.2.0:
    resolution: {integrity: sha512-2Jn/ADrHFFyyniBMi0bncTbJqU78K3/TEYvzvwrWWzD9MYPUa/Gm4r3nOeKFQg/PDM5BsbDYdc8ackW4NTY6NA==}

  mdast-util-to-string@4.0.0:
    resolution: {integrity: sha512-0H44vDimn51F0YwvxSJSm0eCDOJTRlmN0R1yBh4HLj9wiV1Dn0QoXGbvFAWj2hSItVTlCmBF1hqKlIyUBVFLPg==}

  micromark-core-commonmark@2.0.4:
    resolution: {integrity: sha512-wxEeE8v8XVvOrxn1TZj74qYhAtTQsSqufHVS3uNVyT1MupXxhyaHk8/9xDNPh9BmL77QNDgWTkZ3RsYHB2ry7w==}

  micromark-extension-mdx-expression@3.0.1:
    resolution: {integrity: sha512-dD/ADLJ1AeMvSAKBwO22zG22N4ybhe7kFIZ3LsDI0GlsNr2A3KYxb0LdC1u5rj4Nw+CHKY0RVdnHX8vj8ejm4Q==}

  micromark-extension-mdx-jsx@3.0.2:
    resolution: {integrity: sha512-e5+q1DjMh62LZAJOnDraSSbDMvGJ8x3cbjygy2qFEi7HCeUT4BDKCvMozPozcD6WmOt6sVvYDNBKhFSz3kjOVQ==}

  micromark-extension-mdx-md@2.0.0:
    resolution: {integrity: sha512-EpAiszsB3blw4Rpba7xTOUptcFeBFi+6PY8VnJ2hhimH+vCQDirWgsMpz7w1XcZE7LVrSAUGb9VJpG9ghlYvYQ==}

  micromark-extension-mdxjs-esm@3.0.0:
    resolution: {integrity: sha512-DJFl4ZqkErRpq/dAPyeWp15tGrcrrJho1hKK5uBS70BCtfrIFg81sqcTVu3Ta+KD1Tk5vAtBNElWxtAa+m8K9A==}

  micromark-extension-mdxjs@3.0.0:
    resolution: {integrity: sha512-A873fJfhnJ2siZyUrJ31l34Uqwy4xIFmvPY1oj+Ean5PHcPBYzEsvqvWGaWcfEIr11O5Dlw3p2y0tZWpKHDejQ==}

  micromark-factory-destination@2.0.1:
    resolution: {integrity: sha512-Xe6rDdJlkmbFRExpTOmRj9N3MaWmbAgdpSrBQvCFqhezUn4AHqJHbaEnfbVYYiexVSs//tqOdY/DxhjdCiJnIA==}

  micromark-factory-label@2.0.1:
    resolution: {integrity: sha512-VFMekyQExqIW7xIChcXn4ok29YE3rnuyveW3wZQWWqF4Nv9Wk5rgJ99KzPvHjkmPXF93FXIbBp6YdW3t71/7Vg==}

  micromark-factory-mdx-expression@2.0.3:
    resolution: {integrity: sha512-kQnEtA3vzucU2BkrIa8/VaSAsP+EJ3CKOvhMuJgOEGg9KDC6OAY6nSnNDVRiVNRqj7Y4SlSzcStaH/5jge8JdQ==}

  micromark-factory-space@2.1.0:
    resolution: {integrity: sha512-fS8hnLIjnjvdQIj39Geug8wWsR0HrYZ43KShKxNfwT7t2LOHo/LbWZEzEaSOjwtjdCsjoE6syHhonEzW0zv0+Q==}

  micromark-factory-title@2.0.1:
    resolution: {integrity: sha512-5bZ+3CjhAd9eChYTHsjy6TGxpOFSKgKKJPJxr293jTbfry2KDoWkhBb6TcPVB4NmzaPhMs1Frm9AZH7OD4Cjzw==}

  micromark-factory-whitespace@2.0.1:
    resolution: {integrity: sha512-Ob0nuZ3PKt/n0hORHyvoD9uZhr+Za8sFoP+OnMcnWK5lngSzALgQYKMr9RJVOWLqQYuyn6ulqGWSXdwf6F80lQ==}

  micromark-util-character@2.1.1:
    resolution: {integrity: sha512-wv8tdUTJ3thSFFFJKtpYKOYiGP2+v96Hvk4Tu8KpCAsTMs6yi+nVmGh1syvSCsaxz45J6Jbw+9DD6g97+NV67Q==}

  micromark-util-chunked@2.0.1:
    resolution: {integrity: sha512-QUNFEOPELfmvv+4xiNg2sRYeS/P84pTW0TCgP5zc9FpXetHY0ab7SxKyAQCNCc1eK0459uoLI1y5oO5Vc1dbhA==}

  micromark-util-classify-character@2.0.1:
    resolution: {integrity: sha512-K0kHzM6afW/MbeWYWLjoHQv1sgg2Q9EccHEDzSkxiP/EaagNzCm7T/WMKZ3rjMbvIpvBiZgwR3dKMygtA4mG1Q==}

  micromark-util-combine-extensions@2.0.1:
    resolution: {integrity: sha512-OnAnH8Ujmy59JcyZw8JSbK9cGpdVY44NKgSM7E9Eh7DiLS2E9RNQf0dONaGDzEG9yjEl5hcqeIsj4hfRkLH/Bg==}

  micromark-util-decode-numeric-character-reference@2.0.2:
    resolution: {integrity: sha512-ccUbYk6CwVdkmCQMyr64dXz42EfHGkPQlBj5p7YVGzq8I7CtjXZJrubAYezf7Rp+bjPseiROqe7G6foFd+lEuw==}

  micromark-util-decode-string@2.0.1:
    resolution: {integrity: sha512-nDV/77Fj6eH1ynwscYTOsbK7rR//Uj0bZXBwJZRfaLEJ1iGBR6kIfNmlNqaqJf649EP0F3NWNdeJi03elllNUQ==}

  micromark-util-edit-map@1.0.0:
    resolution: {integrity: sha512-Pa2ljlsEL6sVwFaYeyrOLSYbQt73JvGbPOYFq+9AElXiSnfI5Q4465RRZ1sHv7lDjDOnGRDDaqPXqClqWK/q1Q==}

  micromark-util-encode@2.0.1:
    resolution: {integrity: sha512-c3cVx2y4KqUnwopcO9b/SCdo2O67LwJJ/UyqGfbigahfegL9myoEFoDYZgkT7f36T0bLrM9hZTAaAyH+PCAXjw==}

  micromark-util-events-to-acorn@2.0.3:
    resolution: {integrity: sha512-jmsiEIiZ1n7X1Rr5k8wVExBQCg5jy4UXVADItHmNk1zkwEVhBuIUKRu3fqv+hs4nxLISi2DQGlqIOGiFxgbfHg==}

  micromark-util-html-tag-name@2.0.1:
    resolution: {integrity: sha512-2cNEiYDhCWKI+Gs9T0Tiysk136SnR13hhO8yW6BGNyhOC4qYFnwF1nKfD3HFAIXA5c45RrIG1ub11GiXeYd1xA==}

  micromark-util-normalize-identifier@2.0.1:
    resolution: {integrity: sha512-sxPqmo70LyARJs0w2UclACPUUEqltCkJ6PhKdMIDuJ3gSf/Q+/GIe3WKl0Ijb/GyH9lOpUkRAO2wp0GVkLvS9Q==}

  micromark-util-resolve-all@2.0.1:
    resolution: {integrity: sha512-VdQyxFWFT2/FGJgwQnJYbe1jjQoNTS4RjglmSjTUlpUMa95Htx9NHeYW4rGDJzbjvCsl9eLjMQwGeElsqmzcHg==}

  micromark-util-sanitize-uri@2.0.1:
    resolution: {integrity: sha512-9N9IomZ/YuGGZZmQec1MbgxtlgougxTodVwDzzEouPKo3qFWvymFHWcnDi2vzV1ff6kas9ucW+o3yzJK9YB1AQ==}

  micromark-util-subtokenize@2.1.0:
    resolution: {integrity: sha512-XQLu552iSctvnEcgXw6+Sx75GflAPNED1qx7eBJ+wydBb2KCbRZe+NwvIEEMM83uml1+2WSXpBAcp9IUCgCYWA==}

  micromark-util-symbol@2.0.1:
    resolution: {integrity: sha512-vs5t8Apaud9N28kgCrRUdEed4UJ+wWNvicHLPxCa9ENlYuAY31M0ETy5y1vA33YoNPDFTghEbnh6efaE8h4x0Q==}

  micromark-util-types@2.0.3:
    resolution: {integrity: sha512-oxB2Ik03hI0gv+VNn9tnh1t1YEe9MDPptViAEgfdf3YQHsn0pzGTgCdlSCJXcwhqm8phaEuM7zeEu3QQzVBrPg==}

  micromark@4.0.3:
    resolution: {integrity: sha512-oGYfQzHSG5dOMovQcJ3fyTmZlWAWpi0XA0sJwJs+i6OT88o1+Jtw/8z0CmdowSLxGhhFK89rQ/oXph/wN02PNw==}

  ms@2.1.3:
    resolution: {integrity: sha512-6FlzubTLZG3J2a/NVCAleEhjzq5oxgHyaCU9yYXvcLsvoVaHJq/s5xXI6/XXP6tz7R9xAOtHnSO/tXtF3WRTlA==}

  parse-entities@4.0.2:
    resolution: {integrity: sha512-GG2AQYWoLgL877gQIKeRPGO1xF9+eG1ujIb5soS5gPvLQ1y2o8FL90w2QWNdf9I361Mpp7726c+lj3U0qK1uGw==}

  property-information@7.2.0:
    resolution: {integrity: sha512-IAtzIB6sUiWaJYrX9smp3V46pBGbBeLFRGdh25kg1334VcBlD8HzhPeNIWQH9zhGmo2itIe25EHt9dQP7G5hmg==}

  recma-build-jsx@1.0.0:
    resolution: {integrity: sha512-8GtdyqaBcDfva+GUKDr3nev3VpKAhup1+RvkMvUxURHpW7QyIvk9F5wz7Vzo06CEMSilw6uArgRqhpiUcWp8ew==}

  recma-jsx@1.0.1:
    resolution: {integrity: sha512-huSIy7VU2Z5OLv6oFLosQGGDqPqdO1iq6bWNAdhzMxSJP7RAso4fCZ1cKu8j9YHCZf3TPrq4dw3okhrylgcd7w==}
    peerDependencies:
      acorn: ^6.0.0 || ^7.0.0 || ^8.0.0

  recma-parse@1.0.0:
    resolution: {integrity: sha512-OYLsIGBB5Y5wjnSnQW6t3Xg7q3fQ7FWbw/vcXtORTnyaSFscOtABg+7Pnz6YZ6c27fG1/aN8CjfwoUEUIdwqWQ==}

  recma-stringify@1.0.0:
    resolution: {integrity: sha512-cjwII1MdIIVloKvC9ErQ+OgAtwHBmcZ0Bg4ciz78FtbT8In39aAYbaA7zvxQ61xVMSPE8WxhLwLbhif4Js2C+g==}

  rehype-recma@1.0.0:
    resolution: {integrity: sha512-lqA4rGUf1JmacCNWWZx0Wv1dHqMwxzsDWYMTowuplHF3xH0N/MmrZ/G3BDZnzAkRmxDadujCjaKM2hqYdCBOGw==}

  remark-mdx@3.1.1:
    resolution: {integrity: sha512-Pjj2IYlUY3+D8x00UJsIOg5BEvfMyeI+2uLPn9VO9Wg4MEtN/VTIq2NEJQfde9PnX15KgtHyl9S0BcTnWrIuWg==}

  remark-parse@11.0.0:
    resolution: {integrity: sha512-FCxlKLNGknS5ba/1lmpYijMUzX2esxW5xQqjWxw2eHFfS2MSdaHVINFmhjo+qN1WhZhNimq0dZATN9pH0IDrpA==}

  remark-rehype@11.1.2:
    resolution: {integrity: sha512-Dh7l57ianaEoIpzbp0PC9UKAdCSVklD8E5Rpw7ETfbTl3FqcOOgq5q2LVDhgGCkaBv7p24JXikPdvhhmHvKMsw==}

  source-map@0.7.6:
    resolution: {integrity: sha512-i5uvt8C3ikiWeNZSVZNWcfZPItFQOsYTUAOkcUPGd8DqDy1uOUikjt5dG+uRlwyvR108Fb9DOd4GvXfT0N2/uQ==}
    engines: {node: '>= 12'}

  space-separated-tokens@2.0.2:
    resolution: {integrity: sha512-PEGlAwrG8yXGXRjW32fGbg66JAlOAwbObuqVoJpv/mRgoWDQfgH1wDPvtzWyUSNAXBGSk8h755YDbbcEy3SH2Q==}

  stringify-entities@4.0.4:
    resolution: {integrity: sha512-IwfBptatlO+QCJUo19AqvrPNqlVMpW9YEL2LIVY+Rpv2qsjCGxaDLNRgeGsQWJhfItebuJhsGSLjaBbNSQ+ieg==}

  style-to-js@1.1.21:
    resolution: {integrity: sha512-RjQetxJrrUJLQPHbLku6U/ocGtzyjbJMP9lCNK7Ag0CNh690nSH8woqWH9u16nMjYBAok+i7JO1NP2pOy8IsPQ==}

  style-to-object@1.0.14:
    resolution: {integrity: sha512-LIN7rULI0jBscWQYaSswptyderlarFkjQ+t79nzty8tcIAceVomEVlLzH5VP4Cmsv6MtKhs7qaAiwlcp+Mgaxw==}

  trim-lines@3.0.1:
    resolution: {integrity: sha512-kRj8B+YHZCc9kQYdWfJB2/oUl9rA99qbowYYBtr4ui4mZyAQ2JpvVBd/6U2YloATfqBhBTSMhTpgBHtU0Mf3Rg==}

  trough@2.2.0:
    resolution: {integrity: sha512-tmMpK00BjZiUyVyvrBK7knerNgmgvcV/KLVyuma/SC+TQN167GrMRciANTz09+k3zW8L8t60jWO1GpfkZdjTaw==}

  unified@11.0.5:
    resolution: {integrity: sha512-xKvGhPWw3k84Qjh8bI3ZeJjqnyadK+GEFtazSfZv/rKeTkTjOJho6mFqh2SM96iIcZokxiOpg78GazTSg8+KHA==}

  unist-util-is@6.0.1:
    resolution: {integrity: sha512-LsiILbtBETkDz8I9p1dQ0uyRUWuaQzd/cuEeS1hoRSyW5E5XGmTzlwY1OrNzzakGowI9Dr/I8HVaw4hTtnxy8g==}

  unist-util-position-from-estree@2.0.0:
    resolution: {integrity: sha512-KaFVRjoqLyF6YXCbVLNad/eS4+OfPQQn2yOd7zF/h5T/CSL2v8NpN6a5TPvtbXthAGw5nG+PuTtq+DdIZr+cRQ==}

  unist-util-position@5.0.0:
    resolution: {integrity: sha512-fucsC7HjXvkB5R3kTCO7kUjRdrS0BJt3M/FPxmHMBOm8JQi2BsHAHFsy27E0EolP8rp0NzXsJ+jNPyDWvOJZPA==}

  unist-util-stringify-position@4.0.0:
    resolution: {integrity: sha512-0ASV06AAoKCDkS2+xw5RXJywruurpbC4JZSm7nr7MOt1ojAzvyyaO+UxZf18j8FCF6kmzCZKcAgN/yu2gm2XgQ==}

  unist-util-visit-parents@6.0.2:
    resolution: {integrity: sha512-goh1s1TBrqSqukSc8wrjwWhL0hiJxgA8m4kFxGlQ+8FYQ3C/m11FcTs4YYem7V664AhHVvgoQLk890Ssdsr2IQ==}

  unist-util-visit@5.1.0:
    resolution: {integrity: sha512-m+vIdyeCOpdr/QeQCu2EzxX/ohgS8KbnPDgFni4dQsfSCtpz8UqDyY5GjRru8PDKuYn7Fq19j1CQ+nJSsGKOzg==}

  vfile-message@4.0.3:
    resolution: {integrity: sha512-QTHzsGd1EhbZs4AsQ20JX1rC3cOlt/IWJruk893DfLRr57lcnOeMaWG4K0JrRta4mIJZKth2Au3mM3u03/JWKw==}

  vfile@6.0.3:
    resolution: {integrity: sha512-KzIbH/9tXat2u30jf+smMwFCsno4wHVdNmzFyL+T/L3UGqqk6JKfVqOFOZEpZSHADH1k40ab6NUIXZq422ov3Q==}

  zwitch@2.0.4:
    resolution: {integrity: sha512-bXE4cR/kVZhKZX/RjPEflHaKVhUVl85noU3v6b8apfQEc1x4A+zBxjZ4lN8LqGd6WZ3dl98pY4o717VFmoPp+A==}

snapshots:

  '@mdx-js/mdx@3.1.1':
    dependencies:
      '@types/estree': 1.0.9
      '@types/estree-jsx': 1.0.5
      '@types/hast': 3.0.5
      '@types/mdx': 2.0.14
      acorn: 8.19.0
      collapse-white-space: 2.1.0
      devlop: 1.1.0
      estree-util-is-identifier-name: 3.0.0
      estree-util-scope: 1.0.1
      estree-walker: 3.0.3
      hast-util-to-jsx-runtime: 2.3.6
      markdown-extensions: 2.0.0
      recma-build-jsx: 1.0.0
      recma-jsx: 1.0.1(acorn@8.19.0)
      recma-stringify: 1.0.0
      rehype-recma: 1.0.0
      remark-mdx: 3.1.1
      remark-parse: 11.0.0
      remark-rehype: 11.1.2
      source-map: 0.7.6
      unified: 11.0.5
      unist-util-position-from-estree: 2.0.0
      unist-util-stringify-position: 4.0.0
      unist-util-visit: 5.1.0
      vfile: 6.0.3
    transitivePeerDependencies:
      - supports-color

  '@types/debug@4.1.13':
    dependencies:
      '@types/ms': 2.1.0

  '@types/estree-jsx@1.0.5':
    dependencies:
      '@types/estree': 1.0.9

  '@types/estree@1.0.9': {}

  '@types/hast@3.0.5':
    dependencies:
      '@types/unist': 3.0.3

  '@types/mdast@4.0.4':
    dependencies:
      '@types/unist': 3.0.3

  '@types/mdx@2.0.14': {}

  '@types/ms@2.1.0': {}

  '@types/unist@2.0.11': {}

  '@types/unist@3.0.3': {}

  '@ungap/structured-clone@1.4.0': {}

  acorn-jsx@5.3.2(acorn@8.19.0):
    dependencies:
      acorn: 8.19.0

  acorn@8.19.0: {}

  astring@1.9.0: {}

  bail@2.0.2: {}

  ccount@2.0.1: {}

  character-entities-html4@2.1.0: {}

  character-entities-legacy@3.0.0: {}

  character-entities@2.0.2: {}

  character-reference-invalid@2.0.1: {}

  collapse-white-space@2.1.0: {}

  comma-separated-tokens@2.0.3: {}

  debug@4.4.3:
    dependencies:
      ms: 2.1.3

  decode-named-character-reference@1.3.0:
    dependencies:
      character-entities: 2.0.2

  dequal@2.0.3: {}

  devlop@1.1.0:
    dependencies:
      dequal: 2.0.3

  esast-util-from-estree@2.0.0:
    dependencies:
      '@types/estree-jsx': 1.0.5
      devlop: 1.1.0
      estree-util-visit: 2.0.0
      unist-util-position-from-estree: 2.0.0

  esast-util-from-js@2.0.1:
    dependencies:
      '@types/estree-jsx': 1.0.5
      acorn: 8.19.0
      esast-util-from-estree: 2.0.0
      vfile-message: 4.0.3

  estree-util-attach-comments@3.0.0:
    dependencies:
      '@types/estree': 1.0.9

  estree-util-build-jsx@3.0.1:
    dependencies:
      '@types/estree-jsx': 1.0.5
      devlop: 1.1.0
      estree-util-is-identifier-name: 3.0.0
      estree-walker: 3.0.3

  estree-util-is-identifier-name@3.0.0: {}

  estree-util-scope@1.0.1:
    dependencies:
      '@types/estree': 1.0.9
      devlop: 1.1.0

  estree-util-to-js@2.0.0:
    dependencies:
      '@types/estree-jsx': 1.0.5
      astring: 1.9.0
      source-map: 0.7.6

  estree-util-visit@2.0.0:
    dependencies:
      '@types/estree-jsx': 1.0.5
      '@types/unist': 3.0.3

  estree-walker@3.0.3:
    dependencies:
      '@types/estree': 1.0.9

  extend@3.0.2: {}

  hast-util-to-estree@3.1.3:
    dependencies:
      '@types/estree': 1.0.9
      '@types/estree-jsx': 1.0.5
      '@types/hast': 3.0.5
      comma-separated-tokens: 2.0.3
      devlop: 1.1.0
      estree-util-attach-comments: 3.0.0
      estree-util-is-identifier-name: 3.0.0
      hast-util-whitespace: 3.0.0
      mdast-util-mdx-expression: 2.0.1
      mdast-util-mdx-jsx: 3.2.0
      mdast-util-mdxjs-esm: 2.0.1
      property-information: 7.2.0
      space-separated-tokens: 2.0.2
      style-to-js: 1.1.21
      unist-util-position: 5.0.0
      zwitch: 2.0.4
    transitivePeerDependencies:
      - supports-color

  hast-util-to-jsx-runtime@2.3.6:
    dependencies:
      '@types/estree': 1.0.9
      '@types/hast': 3.0.5
      '@types/unist': 3.0.3
      comma-separated-tokens: 2.0.3
      devlop: 1.1.0
      estree-util-is-identifier-name: 3.0.0
      hast-util-whitespace: 3.0.0
      mdast-util-mdx-expression: 2.0.1
      mdast-util-mdx-jsx: 3.2.0
      mdast-util-mdxjs-esm: 2.0.1
      property-information: 7.2.0
      space-separated-tokens: 2.0.2
      style-to-js: 1.1.21
      unist-util-position: 5.0.0
      vfile-message: 4.0.3
    transitivePeerDependencies:
      - supports-color

  hast-util-whitespace@3.0.0:
    dependencies:
      '@types/hast': 3.0.5

  inline-style-parser@0.2.7: {}

  is-alphabetical@2.0.1: {}

  is-alphanumerical@2.0.1:
    dependencies:
      is-alphabetical: 2.0.1
      is-decimal: 2.0.1

  is-decimal@2.0.1: {}

  is-hexadecimal@2.0.1: {}

  is-plain-obj@4.1.0: {}

  longest-streak@3.1.0: {}

  markdown-extensions@2.0.0: {}

  mdast-util-from-markdown@2.1.0:
    dependencies:
      '@types/mdast': 4.0.4
      '@types/unist': 3.0.3
      decode-named-character-reference: 1.3.0
      devlop: 1.1.0
      mdast-util-to-string: 4.0.0
      micromark: 4.0.3
      micromark-util-decode-numeric-character-reference: 2.0.2
      micromark-util-decode-string: 2.0.1
      micromark-util-normalize-identifier: 2.0.1
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3
      unist-util-stringify-position: 4.0.0
    transitivePeerDependencies:
      - supports-color

  mdast-util-mdx-expression@2.0.1:
    dependencies:
      '@types/estree-jsx': 1.0.5
      '@types/hast': 3.0.5
      '@types/mdast': 4.0.4
      devlop: 1.1.0
      mdast-util-from-markdown: 2.1.0
      mdast-util-to-markdown: 2.2.0
    transitivePeerDependencies:
      - supports-color

  mdast-util-mdx-jsx@3.2.0:
    dependencies:
      '@types/estree-jsx': 1.0.5
      '@types/hast': 3.0.5
      '@types/mdast': 4.0.4
      '@types/unist': 3.0.3
      ccount: 2.0.1
      devlop: 1.1.0
      mdast-util-from-markdown: 2.1.0
      mdast-util-to-markdown: 2.2.0
      parse-entities: 4.0.2
      stringify-entities: 4.0.4
      unist-util-stringify-position: 4.0.0
      vfile-message: 4.0.3
    transitivePeerDependencies:
      - supports-color

  mdast-util-mdx@3.0.0:
    dependencies:
      mdast-util-from-markdown: 2.1.0
      mdast-util-mdx-expression: 2.0.1
      mdast-util-mdx-jsx: 3.2.0
      mdast-util-mdxjs-esm: 2.0.1
      mdast-util-to-markdown: 2.2.0
    transitivePeerDependencies:
      - supports-color

  mdast-util-mdxjs-esm@2.0.1:
    dependencies:
      '@types/estree-jsx': 1.0.5
      '@types/hast': 3.0.5
      '@types/mdast': 4.0.4
      devlop: 1.1.0
      mdast-util-from-markdown: 2.1.0
      mdast-util-to-markdown: 2.2.0
    transitivePeerDependencies:
      - supports-color

  mdast-util-phrasing@4.1.0:
    dependencies:
      '@types/mdast': 4.0.4
      unist-util-is: 6.0.1

  mdast-util-to-hast@13.2.1:
    dependencies:
      '@types/hast': 3.0.5
      '@types/mdast': 4.0.4
      '@ungap/structured-clone': 1.4.0
      devlop: 1.1.0
      micromark-util-sanitize-uri: 2.0.1
      trim-lines: 3.0.1
      unist-util-position: 5.0.0
      unist-util-visit: 5.1.0
      vfile: 6.0.3

  mdast-util-to-markdown@2.2.0:
    dependencies:
      '@types/mdast': 4.0.4
      '@types/unist': 3.0.3
      longest-streak: 3.1.0
      mdast-util-phrasing: 4.1.0
      mdast-util-to-string: 4.0.0
      micromark-util-character: 2.1.1
      micromark-util-classify-character: 2.0.1
      micromark-util-decode-string: 2.0.1
      micromark-util-html-tag-name: 2.0.1
      unist-util-visit: 5.1.0
      zwitch: 2.0.4

  mdast-util-to-string@4.0.0:
    dependencies:
      '@types/mdast': 4.0.4

  micromark-core-commonmark@2.0.4:
    dependencies:
      decode-named-character-reference: 1.3.0
      devlop: 1.1.0
      micromark-factory-destination: 2.0.1
      micromark-factory-label: 2.0.1
      micromark-factory-space: 2.1.0
      micromark-factory-title: 2.0.1
      micromark-factory-whitespace: 2.0.1
      micromark-util-character: 2.1.1
      micromark-util-chunked: 2.0.1
      micromark-util-classify-character: 2.0.1
      micromark-util-edit-map: 1.0.0
      micromark-util-html-tag-name: 2.0.1
      micromark-util-normalize-identifier: 2.0.1
      micromark-util-resolve-all: 2.0.1
      micromark-util-subtokenize: 2.1.0
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-extension-mdx-expression@3.0.1:
    dependencies:
      '@types/estree': 1.0.9
      devlop: 1.1.0
      micromark-factory-mdx-expression: 2.0.3
      micromark-factory-space: 2.1.0
      micromark-util-character: 2.1.1
      micromark-util-events-to-acorn: 2.0.3
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-extension-mdx-jsx@3.0.2:
    dependencies:
      '@types/estree': 1.0.9
      devlop: 1.1.0
      estree-util-is-identifier-name: 3.0.0
      micromark-factory-mdx-expression: 2.0.3
      micromark-factory-space: 2.1.0
      micromark-util-character: 2.1.1
      micromark-util-events-to-acorn: 2.0.3
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3
      vfile-message: 4.0.3

  micromark-extension-mdx-md@2.0.0:
    dependencies:
      micromark-util-types: 2.0.3

  micromark-extension-mdxjs-esm@3.0.0:
    dependencies:
      '@types/estree': 1.0.9
      devlop: 1.1.0
      micromark-core-commonmark: 2.0.4
      micromark-util-character: 2.1.1
      micromark-util-events-to-acorn: 2.0.3
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3
      unist-util-position-from-estree: 2.0.0
      vfile-message: 4.0.3

  micromark-extension-mdxjs@3.0.0:
    dependencies:
      acorn: 8.19.0
      acorn-jsx: 5.3.2(acorn@8.19.0)
      micromark-extension-mdx-expression: 3.0.1
      micromark-extension-mdx-jsx: 3.0.2
      micromark-extension-mdx-md: 2.0.0
      micromark-extension-mdxjs-esm: 3.0.0
      micromark-util-combine-extensions: 2.0.1
      micromark-util-types: 2.0.3

  micromark-factory-destination@2.0.1:
    dependencies:
      micromark-util-character: 2.1.1
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-factory-label@2.0.1:
    dependencies:
      devlop: 1.1.0
      micromark-util-character: 2.1.1
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-factory-mdx-expression@2.0.3:
    dependencies:
      '@types/estree': 1.0.9
      devlop: 1.1.0
      micromark-factory-space: 2.1.0
      micromark-util-character: 2.1.1
      micromark-util-events-to-acorn: 2.0.3
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3
      unist-util-position-from-estree: 2.0.0
      vfile-message: 4.0.3

  micromark-factory-space@2.1.0:
    dependencies:
      micromark-util-character: 2.1.1
      micromark-util-types: 2.0.3

  micromark-factory-title@2.0.1:
    dependencies:
      micromark-factory-space: 2.1.0
      micromark-util-character: 2.1.1
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-factory-whitespace@2.0.1:
    dependencies:
      micromark-factory-space: 2.1.0
      micromark-util-character: 2.1.1
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-util-character@2.1.1:
    dependencies:
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-util-chunked@2.0.1:
    dependencies:
      micromark-util-symbol: 2.0.1

  micromark-util-classify-character@2.0.1:
    dependencies:
      micromark-util-character: 2.1.1
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-util-combine-extensions@2.0.1:
    dependencies:
      micromark-util-chunked: 2.0.1
      micromark-util-types: 2.0.3

  micromark-util-decode-numeric-character-reference@2.0.2:
    dependencies:
      micromark-util-symbol: 2.0.1

  micromark-util-decode-string@2.0.1:
    dependencies:
      decode-named-character-reference: 1.3.0
      micromark-util-character: 2.1.1
      micromark-util-decode-numeric-character-reference: 2.0.2
      micromark-util-symbol: 2.0.1

  micromark-util-edit-map@1.0.0:
    dependencies:
      micromark-util-types: 2.0.3

  micromark-util-encode@2.0.1: {}

  micromark-util-events-to-acorn@2.0.3:
    dependencies:
      '@types/estree': 1.0.9
      '@types/unist': 3.0.3
      devlop: 1.1.0
      estree-util-visit: 2.0.0
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3
      vfile-message: 4.0.3

  micromark-util-html-tag-name@2.0.1: {}

  micromark-util-normalize-identifier@2.0.1:
    dependencies:
      micromark-util-symbol: 2.0.1

  micromark-util-resolve-all@2.0.1:
    dependencies:
      micromark-util-types: 2.0.3

  micromark-util-sanitize-uri@2.0.1:
    dependencies:
      micromark-util-character: 2.1.1
      micromark-util-encode: 2.0.1
      micromark-util-symbol: 2.0.1

  micromark-util-subtokenize@2.1.0:
    dependencies:
      devlop: 1.1.0
      micromark-util-chunked: 2.0.1
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3

  micromark-util-symbol@2.0.1: {}

  micromark-util-types@2.0.3: {}

  micromark@4.0.3:
    dependencies:
      '@types/debug': 4.1.13
      debug: 4.4.3
      decode-named-character-reference: 1.3.0
      devlop: 1.1.0
      micromark-core-commonmark: 2.0.4
      micromark-factory-space: 2.1.0
      micromark-util-character: 2.1.1
      micromark-util-chunked: 2.0.1
      micromark-util-combine-extensions: 2.0.1
      micromark-util-decode-numeric-character-reference: 2.0.2
      micromark-util-edit-map: 1.0.0
      micromark-util-encode: 2.0.1
      micromark-util-normalize-identifier: 2.0.1
      micromark-util-resolve-all: 2.0.1
      micromark-util-sanitize-uri: 2.0.1
      micromark-util-subtokenize: 2.1.0
      micromark-util-symbol: 2.0.1
      micromark-util-types: 2.0.3
    transitivePeerDependencies:
      - supports-color

  ms@2.1.3: {}

  parse-entities@4.0.2:
    dependencies:
      '@types/unist': 2.0.11
      character-entities-legacy: 3.0.0
      character-reference-invalid: 2.0.1
      decode-named-character-reference: 1.3.0
      is-alphanumerical: 2.0.1
      is-decimal: 2.0.1
      is-hexadecimal: 2.0.1

  property-information@7.2.0: {}

  recma-build-jsx@1.0.0:
    dependencies:
      '@types/estree': 1.0.9
      estree-util-build-jsx: 3.0.1
      vfile: 6.0.3

  recma-jsx@1.0.1(acorn@8.19.0):
    dependencies:
      acorn: 8.19.0
      acorn-jsx: 5.3.2(acorn@8.19.0)
      estree-util-to-js: 2.0.0
      recma-parse: 1.0.0
      recma-stringify: 1.0.0
      unified: 11.0.5

  recma-parse@1.0.0:
    dependencies:
      '@types/estree': 1.0.9
      esast-util-from-js: 2.0.1
      unified: 11.0.5
      vfile: 6.0.3

  recma-stringify@1.0.0:
    dependencies:
      '@types/estree': 1.0.9
      estree-util-to-js: 2.0.0
      unified: 11.0.5
      vfile: 6.0.3

  rehype-recma@1.0.0:
    dependencies:
      '@types/estree': 1.0.9
      '@types/hast': 3.0.5
      hast-util-to-estree: 3.1.3
    transitivePeerDependencies:
      - supports-color

  remark-mdx@3.1.1:
    dependencies:
      mdast-util-mdx: 3.0.0
      micromark-extension-mdxjs: 3.0.0
    transitivePeerDependencies:
      - supports-color

  remark-parse@11.0.0:
    dependencies:
      '@types/mdast': 4.0.4
      mdast-util-from-markdown: 2.1.0
      micromark-util-types: 2.0.3
      unified: 11.0.5
    transitivePeerDependencies:
      - supports-color

  remark-rehype@11.1.2:
    dependencies:
      '@types/hast': 3.0.5
      '@types/mdast': 4.0.4
      mdast-util-to-hast: 13.2.1
      unified: 11.0.5
      vfile: 6.0.3

  source-map@0.7.6: {}

  space-separated-tokens@2.0.2: {}

  stringify-entities@4.0.4:
    dependencies:
      character-entities-html4: 2.1.0
      character-entities-legacy: 3.0.0

  style-to-js@1.1.21:
    dependencies:
      style-to-object: 1.0.14

  style-to-object@1.0.14:
    dependencies:
      inline-style-parser: 0.2.7

  trim-lines@3.0.1: {}

  trough@2.2.0: {}

  unified@11.0.5:
    dependencies:
      '@types/unist': 3.0.3
      bail: 2.0.2
      devlop: 1.1.0
      extend: 3.0.2
      is-plain-obj: 4.1.0
      trough: 2.2.0
      vfile: 6.0.3

  unist-util-is@6.0.1:
    dependencies:
      '@types/unist': 3.0.3

  unist-util-position-from-estree@2.0.0:
    dependencies:
      '@types/unist': 3.0.3

  unist-util-position@5.0.0:
    dependencies:
      '@types/unist': 3.0.3

  unist-util-stringify-position@4.0.0:
    dependencies:
      '@types/unist': 3.0.3

  unist-util-visit-parents@6.0.2:
    dependencies:
      '@types/unist': 3.0.3
      unist-util-is: 6.0.1

  unist-util-visit@5.1.0:
    dependencies:
      '@types/unist': 3.0.3
      unist-util-is: 6.0.1
      unist-util-visit-parents: 6.0.2

  vfile-message@4.0.3:
    dependencies:
      '@types/unist': 3.0.3
      unist-util-stringify-position: 4.0.0

  vfile@6.0.3:
    dependencies:
      '@types/unist': 3.0.3
      vfile-message: 4.0.3

  zwitch@2.0.4: {}
"#;
