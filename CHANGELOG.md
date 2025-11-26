# Changelog

## [0.1.34](https://github.com/AllexVeldman/pyoci/compare/v0.1.33...v0.1.34) (2025-11-26)


### Features

* Bind to IPv6 `[::]` ([#322](https://github.com/AllexVeldman/pyoci/issues/322)) ([e9cbb86](https://github.com/AllexVeldman/pyoci/commit/e9cbb862f9dc8a05b21119e7719fda608d6f3499))


### Dependency Updates

* update tokio-tracing monorepo ([#324](https://github.com/AllexVeldman/pyoci/issues/324)) ([527ed72](https://github.com/AllexVeldman/pyoci/commit/527ed727cb6aec950f3c6a2c6db685815685f4d8))

## [0.1.33](https://github.com/AllexVeldman/pyoci/compare/v0.1.32...v0.1.33) (2025-11-25)


### Bug Fixes

* **auth:** 502 when authenticating with `registry-1.docker.io` ([#321](https://github.com/AllexVeldman/pyoci/issues/321)) ([a78fe98](https://github.com/AllexVeldman/pyoci/commit/a78fe98c9f5501d2405ceae7654e04b80bb4aa12))


### Dependency Updates

* update actions/checkout action to v6 ([#316](https://github.com/AllexVeldman/pyoci/issues/316)) ([7211f32](https://github.com/AllexVeldman/pyoci/commit/7211f3253ea0c0afbc27e1d62471ff41052e7fd1))
* update axum monorepo ([#315](https://github.com/AllexVeldman/pyoci/issues/315)) ([a158fee](https://github.com/AllexVeldman/pyoci/commit/a158fee9bc78c3e0ca078a63a362226ba3a9c52d))
* update pre-commit hook rhysd/actionlint to v1.7.9 ([#317](https://github.com/AllexVeldman/pyoci/issues/317)) ([dbbdcf0](https://github.com/AllexVeldman/pyoci/commit/dbbdcf0d735c1e6e904c63a3ff9167983692e775))
* update rust crate bytes to v1.11.0 ([#314](https://github.com/AllexVeldman/pyoci/issues/314)) ([f124825](https://github.com/AllexVeldman/pyoci/commit/f124825beba622303de3e6233d4668d678ffc0f8))
* update rust crate http to v1.4.0 ([#320](https://github.com/AllexVeldman/pyoci/issues/320)) ([5e1b7e9](https://github.com/AllexVeldman/pyoci/commit/5e1b7e93458cb4a4ecc65342e2929e0a08c01cdb))
* update rust docker tag to v1.91.1 ([#313](https://github.com/AllexVeldman/pyoci/issues/313)) ([999f3b9](https://github.com/AllexVeldman/pyoci/commit/999f3b9c6b6407e82e10d5b19b046090a52ee945))

## [0.1.32](https://github.com/AllexVeldman/pyoci/compare/v0.1.31...v0.1.32) (2025-11-04)


### Features

* Stream response for file download ([#312](https://github.com/AllexVeldman/pyoci/issues/312)) ([1fe44ac](https://github.com/AllexVeldman/pyoci/commit/1fe44acf7800abcfb2c993c4e779b63e8e78ec54))


### Documentation

* Add documentation for package sub-paths ([#305](https://github.com/AllexVeldman/pyoci/issues/305)) ([2cafc33](https://github.com/AllexVeldman/pyoci/commit/2cafc33661a2a73a61063e2e76b12ac78f42a50d))


### Miscellaneous Chores

* Add abandonments to Renovate config ([#301](https://github.com/AllexVeldman/pyoci/issues/301)) ([397c770](https://github.com/AllexVeldman/pyoci/commit/397c770a702ebb18f115535932b373d8507c7f32))
* Remove unneeded clone ([#308](https://github.com/AllexVeldman/pyoci/issues/308)) ([5bdaac7](https://github.com/AllexVeldman/pyoci/commit/5bdaac7dd433cb175c4cdad7675ccf859cd95af3))


### Code Refactoring

* Make codebase pass `clippy::pedantic` ([#306](https://github.com/AllexVeldman/pyoci/issues/306)) ([02e7051](https://github.com/AllexVeldman/pyoci/commit/02e7051ddcdd3e6188d1908d0207233134abbf29))


### Continuous Integration

* Provide a Python version and pip install poetry ([#303](https://github.com/AllexVeldman/pyoci/issues/303)) ([4d18ba9](https://github.com/AllexVeldman/pyoci/commit/4d18ba9edeb81a5a2077d75bc444a4613115cf33))


### Dependency Updates

* update python docker tag to v3.14 ([#304](https://github.com/AllexVeldman/pyoci/issues/304)) ([038e554](https://github.com/AllexVeldman/pyoci/commit/038e554298fb0e3c462bfea183f98737a1f73879))
* update rust crate axum-extra to 0.12.0 ([#309](https://github.com/AllexVeldman/pyoci/issues/309)) ([83c1567](https://github.com/AllexVeldman/pyoci/commit/83c1567330cd618b84f802fced489e876e000edf))
* update rust crate base16ct to 0.3.0 ([#274](https://github.com/AllexVeldman/pyoci/issues/274)) ([3af868d](https://github.com/AllexVeldman/pyoci/commit/3af868d65e68b95e52c0c160a7d4bd33f22f5e8a))
* update rust crate opentelemetry-proto to 0.31.0 ([#276](https://github.com/AllexVeldman/pyoci/issues/276)) ([4932445](https://github.com/AllexVeldman/pyoci/commit/4932445a563000646823daea552eb079b823d48b))
* update rust crate tokio-util to v0.7.17 ([#311](https://github.com/AllexVeldman/pyoci/issues/311)) ([57ca848](https://github.com/AllexVeldman/pyoci/commit/57ca8483b6dcf762c8006fb3bb1b2a386f1569d0))
* update rust docker tag to v1.91.0 ([#307](https://github.com/AllexVeldman/pyoci/issues/307)) ([0dfe645](https://github.com/AllexVeldman/pyoci/commit/0dfe64545529ecaef85444b90a52ab73dec41ae4))

## [0.1.31](https://github.com/AllexVeldman/pyoci/compare/v0.1.30...v0.1.31) (2025-10-27)


### Features

* Support OCI sub-namespaces ([#297](https://github.com/AllexVeldman/pyoci/issues/297)) ([4829ce8](https://github.com/AllexVeldman/pyoci/commit/4829ce87f3462636131bd0a55022769f2b84b5f0)), closes [#293](https://github.com/AllexVeldman/pyoci/issues/293)


### Bug Fixes

* install fails when `PYOCI_PATH` ends with `/` ([#300](https://github.com/AllexVeldman/pyoci/issues/300)) ([d390f2e](https://github.com/AllexVeldman/pyoci/commit/d390f2e560858c3bb35572a9675d525b78191ad9))


### Miscellaneous Chores

* **deps:** update sonarsource/sonarqube-scan-action action to v6 ([#285](https://github.com/AllexVeldman/pyoci/issues/285)) ([f6e27f1](https://github.com/AllexVeldman/pyoci/commit/f6e27f154022d5a3a5a432c9f0c2b91e58a80ac7))
* Make `Env` static ([#299](https://github.com/AllexVeldman/pyoci/issues/299)) ([0864633](https://github.com/AllexVeldman/pyoci/commit/08646338d495b664e1f0be360e9146b28791b26d))
* Update renovate config ([#288](https://github.com/AllexVeldman/pyoci/issues/288)) ([33a516c](https://github.com/AllexVeldman/pyoci/commit/33a516c5904bc31f2689292c1ba11ce97d79c150))


### Dependency Updates

* update astral-sh/setup-uv action to v6 ([#283](https://github.com/AllexVeldman/pyoci/issues/283)) ([900e5a4](https://github.com/AllexVeldman/pyoci/commit/900e5a46e34f8afd7099210fbc61f112ff86e34d))
* update astral-sh/setup-uv action to v7 ([#289](https://github.com/AllexVeldman/pyoci/issues/289)) ([33987f5](https://github.com/AllexVeldman/pyoci/commit/33987f5391042f769e0e067198e34431ba6bf0e2))
* update axum monorepo ([#290](https://github.com/AllexVeldman/pyoci/issues/290)) ([2d7132d](https://github.com/AllexVeldman/pyoci/commit/2d7132d3fcb6c84cb11d17d79d985ea119abf331))
* update docker.io/library/registry docker tag to v3 ([#284](https://github.com/AllexVeldman/pyoci/issues/284)) ([458d5fd](https://github.com/AllexVeldman/pyoci/commit/458d5fd3ee9079fcddf1bab3501f31cd2b2005c2))
* update extractions/setup-just action to v3 ([#287](https://github.com/AllexVeldman/pyoci/issues/287)) ([26530b9](https://github.com/AllexVeldman/pyoci/commit/26530b9159007e32da90c6ca6fee27f7c544a2b5))
* update github artifact actions ([31abaf0](https://github.com/AllexVeldman/pyoci/commit/31abaf0ba2bf4c64a53b05702c70af5630d2fcda))
* update github artifact actions (major) ([#298](https://github.com/AllexVeldman/pyoci/issues/298)) ([31abaf0](https://github.com/AllexVeldman/pyoci/commit/31abaf0ba2bf4c64a53b05702c70af5630d2fcda))
* update pre-commit hook rhysd/actionlint to v1.7.8 ([#292](https://github.com/AllexVeldman/pyoci/issues/292)) ([fdfe5cb](https://github.com/AllexVeldman/pyoci/commit/fdfe5cb50352068bfae49b8921f29004cf0d8d36))
* update rust crate indoc to v2.0.7 ([#296](https://github.com/AllexVeldman/pyoci/issues/296)) ([67ccffa](https://github.com/AllexVeldman/pyoci/commit/67ccffafed94f28b3ef05ffafe8da0609fb89704))
* update rust crate oci-spec to v0.8.3 ([#291](https://github.com/AllexVeldman/pyoci/issues/291)) ([f9c8b70](https://github.com/AllexVeldman/pyoci/commit/f9c8b702f9be2afdbc296a3ead1137f29fb47770))
* update rust crate reqwest to v0.12.24 ([#294](https://github.com/AllexVeldman/pyoci/issues/294)) ([7003c80](https://github.com/AllexVeldman/pyoci/commit/7003c803801eb21724be0c64762c2811066270c6))
* update rust crate tokio to v1.48.0 ([#295](https://github.com/AllexVeldman/pyoci/issues/295)) ([19a227a](https://github.com/AllexVeldman/pyoci/commit/19a227a13c44b319e2363ebb4ee746f938b804a1))
* update rust docker tag to v1.90.0 ([#273](https://github.com/AllexVeldman/pyoci/issues/273)) ([f925519](https://github.com/AllexVeldman/pyoci/commit/f9255199fc3e6d12af7952a3862fbc5ca78890a7))

## [0.1.30](https://github.com/AllexVeldman/pyoci/compare/v0.1.29...v0.1.30) (2025-09-29)


### Features

* **release:** Allow building for multiple architectures ([#266](https://github.com/AllexVeldman/pyoci/issues/266)) ([90aebe9](https://github.com/AllexVeldman/pyoci/commit/90aebe9ecf887a622416cbba25f53fa4efecdcc8))


### Bug Fixes

* **release:** Tag workflow not triggered ([#260](https://github.com/AllexVeldman/pyoci/issues/260)) ([62a0088](https://github.com/AllexVeldman/pyoci/commit/62a0088e7d91b91d08d836f40bfabe40600b0e3e))


### Miscellaneous Chores

* Configure Renovate ([#272](https://github.com/AllexVeldman/pyoci/issues/272)) ([fd255a3](https://github.com/AllexVeldman/pyoci/commit/fd255a3d5b4761f6a0b7dd5a096d8376d2ff5f7a))
* **deps:** update actions/checkout action to v5 ([#279](https://github.com/AllexVeldman/pyoci/issues/279)) ([174574c](https://github.com/AllexVeldman/pyoci/commit/174574cfb73c3c028910e30244e314f0d1cabd0c))
* **deps:** update actions/download-artifact action to v5 ([#281](https://github.com/AllexVeldman/pyoci/issues/281)) ([6a06cd6](https://github.com/AllexVeldman/pyoci/commit/6a06cd65442b618cfd564817865e0e5386d89444))
* **deps:** update actions/setup-python action to v6 ([#280](https://github.com/AllexVeldman/pyoci/issues/280)) ([813d799](https://github.com/AllexVeldman/pyoci/commit/813d7992414d90fc9cb755fc98e280a54d72ee29))
* **deps:** update amannn/action-semantic-pull-request action to v6 ([#282](https://github.com/AllexVeldman/pyoci/issues/282)) ([68bab75](https://github.com/AllexVeldman/pyoci/commit/68bab758b250e9c71e9c5e46e8e1c83f4b6f4e19))
* Switch to Renovate ([#278](https://github.com/AllexVeldman/pyoci/issues/278)) ([b4b2127](https://github.com/AllexVeldman/pyoci/commit/b4b212703e4bc3f93ecb5106263395ef69ce6eb2))


### Code Refactoring

* Split the OCI implementation from PyOci. ([#261](https://github.com/AllexVeldman/pyoci/issues/261)) ([8a8b946](https://github.com/AllexVeldman/pyoci/commit/8a8b94699ac69b0e24a339ea8865053874b39c3a))


### Dependency Updates

* bump anyhow from 1.0.98 to 1.0.99 ([#252](https://github.com/AllexVeldman/pyoci/issues/252)) ([bfd95a0](https://github.com/AllexVeldman/pyoci/commit/bfd95a0776beb50be261030379d11c7b2656bc1f))
* bump anyhow from 1.0.99 to 1.0.100 ([#265](https://github.com/AllexVeldman/pyoci/issues/265)) ([24ae0ae](https://github.com/AllexVeldman/pyoci/commit/24ae0ae2a9c4542803965aef76e8d7dfd26e1494))
* bump async-trait from 0.1.88 to 0.1.89 ([#254](https://github.com/AllexVeldman/pyoci/issues/254)) ([a5e5564](https://github.com/AllexVeldman/pyoci/commit/a5e5564c8d8479314b316ca0c5d8ff1f8242f671))
* bump axum from 0.8.4 to 0.8.5 ([#271](https://github.com/AllexVeldman/pyoci/issues/271)) ([a257776](https://github.com/AllexVeldman/pyoci/commit/a2577763315ceeffb9bb2dd633bbd22e42ca5971))
* bump axum-extra from 0.10.1 to 0.10.2 ([#268](https://github.com/AllexVeldman/pyoci/issues/268)) ([efcafd6](https://github.com/AllexVeldman/pyoci/commit/efcafd6dd89663abfbffa307f061ef2f6de24b05))
* bump oci-spec from 0.8.1 to 0.8.2 ([#253](https://github.com/AllexVeldman/pyoci/issues/253)) ([86ad1d1](https://github.com/AllexVeldman/pyoci/commit/86ad1d1df9d9e8741e2293f7ba03f5c4cb0603bb))
* bump serde from 1.0.219 to 1.0.223 ([#255](https://github.com/AllexVeldman/pyoci/issues/255)) ([4f2da87](https://github.com/AllexVeldman/pyoci/commit/4f2da874f5cb6f27dde52bcd75289f3054817f88))
* bump serde from 1.0.223 to 1.0.228 ([#269](https://github.com/AllexVeldman/pyoci/issues/269)) ([da9f3b3](https://github.com/AllexVeldman/pyoci/commit/da9f3b3aca4337d1c3ad9b73ac3ae8daefe93755))
* bump serde_json from 1.0.143 to 1.0.145 ([#264](https://github.com/AllexVeldman/pyoci/issues/264)) ([fa231ab](https://github.com/AllexVeldman/pyoci/commit/fa231abe4f3038581fb4fb483bc9deb161796f77))
* bump time from 0.3.41 to 0.3.44 ([#263](https://github.com/AllexVeldman/pyoci/issues/263)) ([171a30c](https://github.com/AllexVeldman/pyoci/commit/171a30cb2079eba062aaad81cdbf0f9f2f829a07))
* bump url from 2.5.4 to 2.5.7 ([#262](https://github.com/AllexVeldman/pyoci/issues/262)) ([8a81a63](https://github.com/AllexVeldman/pyoci/commit/8a81a63d8193e13bd5b5314c9f7ca7c33b47b8b7))

## [0.1.29](https://github.com/AllexVeldman/pyoci/compare/v0.1.28...v0.1.29) (2025-09-15)


### Bug Fixes

* Panic when PYOCI_PATH is empty or root ([#256](https://github.com/AllexVeldman/pyoci/issues/256)) ([fb90f17](https://github.com/AllexVeldman/pyoci/commit/fb90f170299c8eae40c90d27ac2460843004dd73)), closes [#251](https://github.com/AllexVeldman/pyoci/issues/251)


### Miscellaneous Chores

* **ci:** Be explicit about secrets used in reusable workflow ([#245](https://github.com/AllexVeldman/pyoci/issues/245)) ([3166b7e](https://github.com/AllexVeldman/pyoci/commit/3166b7ef953643403115d735115015f93cb48ac2))
* **docker:** Add `--no-install-recommends` and clean cache ([#247](https://github.com/AllexVeldman/pyoci/issues/247)) ([a80c253](https://github.com/AllexVeldman/pyoci/commit/a80c2537dc9b4f466b3e65d78b276aac07516064))
* **docker:** Don't cache when building a release ([#248](https://github.com/AllexVeldman/pyoci/issues/248)) ([d083772](https://github.com/AllexVeldman/pyoci/commit/d0837724aba454ed5a2d5f0aa643554350422f97))


### Code Refactoring

* **ci:** Trigger release build/deploy on tag ([#243](https://github.com/AllexVeldman/pyoci/issues/243)) ([41edf77](https://github.com/AllexVeldman/pyoci/commit/41edf7720617173811129664611e24513be2d55f))

## [0.1.28](https://github.com/AllexVeldman/pyoci/compare/v0.1.27...v0.1.28) (2025-09-10)


### Documentation

* **examples:** add `uv` examples ([#224](https://github.com/AllexVeldman/pyoci/issues/224)) ([8bc5dfd](https://github.com/AllexVeldman/pyoci/commit/8bc5dfd9505cd964cc04eee61e81f043f8093afc))
* Fix typo ([#240](https://github.com/AllexVeldman/pyoci/issues/240)) ([f69e3be](https://github.com/AllexVeldman/pyoci/commit/f69e3bec877e1fd20cc2df0d39ccf91ba4e5cf04))


### Dependency Updates

* bump http from 1.2.0 to 1.3.1 ([#226](https://github.com/AllexVeldman/pyoci/issues/226)) ([f69b241](https://github.com/AllexVeldman/pyoci/commit/f69b241b330318724661b28d7efab8b25c8a962d))
* bump oci-spec from 0.7.1 to 0.8.1 ([#227](https://github.com/AllexVeldman/pyoci/issues/227)) ([c265b02](https://github.com/AllexVeldman/pyoci/commit/c265b021144f43ff4dfebc8baf0dc62970051677))
* bump rand from 0.9.1 to 0.9.2 ([#235](https://github.com/AllexVeldman/pyoci/issues/235)) ([2b01e2e](https://github.com/AllexVeldman/pyoci/commit/2b01e2e8ddddf03f00dd6f5e5d5d0b22b12d8cb6))
* bump reqwest from 0.12.15 to 0.12.20 ([#228](https://github.com/AllexVeldman/pyoci/issues/228)) ([fb54ad3](https://github.com/AllexVeldman/pyoci/commit/fb54ad33ae6fceafb28a9791486fe140736c3371))
* bump reqwest from 0.12.20 to 0.12.22 ([#233](https://github.com/AllexVeldman/pyoci/issues/233)) ([75f6b28](https://github.com/AllexVeldman/pyoci/commit/75f6b28bff47fcb7d5188b3b30eb2e5fa0208c33))
* bump reqwest from 0.12.22 to 0.12.23 ([#239](https://github.com/AllexVeldman/pyoci/issues/239)) ([d66d5a5](https://github.com/AllexVeldman/pyoci/commit/d66d5a5591030e24a4b645c3015ce41e74b1bbab))
* bump serde_json from 1.0.140 to 1.0.141 ([#234](https://github.com/AllexVeldman/pyoci/issues/234)) ([3752d15](https://github.com/AllexVeldman/pyoci/commit/3752d151dedb9bb5f6441d9cb31bfc0fe1362448))
* bump serde_json from 1.0.141 to 1.0.143 ([#241](https://github.com/AllexVeldman/pyoci/issues/241)) ([38f29dc](https://github.com/AllexVeldman/pyoci/commit/38f29dc7d3dd95e939a1bbd5907acb818cd42629))
* bump tokio from 1.45.1 to 1.47.0 ([#232](https://github.com/AllexVeldman/pyoci/issues/232)) ([3949bf1](https://github.com/AllexVeldman/pyoci/commit/3949bf1363b7a8d1c1115e9cacfa7d7fe5d71db5))
* bump tokio from 1.47.0 to 1.47.1 ([#236](https://github.com/AllexVeldman/pyoci/issues/236)) ([dd3ef65](https://github.com/AllexVeldman/pyoci/commit/dd3ef65f9988e5e6e3e57bbbfd92528f056ccdc1))
* bump tokio-util from 0.7.15 to 0.7.16 ([#238](https://github.com/AllexVeldman/pyoci/issues/238)) ([d60e7c3](https://github.com/AllexVeldman/pyoci/commit/d60e7c3c51cb8bb0c7e98ec0838a0f01e8700b29))
* bump tracing-core from 0.1.33 to 0.1.34 ([#229](https://github.com/AllexVeldman/pyoci/issues/229)) ([727e1b4](https://github.com/AllexVeldman/pyoci/commit/727e1b43844a00669615795b29ae77f25fa02c6f))
* bump tracing-subscriber from 0.3.19 to 0.3.20 in the cargo group ([#242](https://github.com/AllexVeldman/pyoci/issues/242)) ([092e655](https://github.com/AllexVeldman/pyoci/commit/092e6552e330b2915fe021ae26a7e1b40c627a4b))

## [0.1.27](https://github.com/AllexVeldman/pyoci/compare/v0.1.26...v0.1.27) (2025-06-13)


### Documentation

* Update coverage badge ([#210](https://github.com/AllexVeldman/pyoci/issues/210)) ([fe070dc](https://github.com/AllexVeldman/pyoci/commit/fe070dc74af85925d5c3fd655eedccbf29985da5))


### Miscellaneous Chores

* **deps:** Remove opentelemetry group ([#217](https://github.com/AllexVeldman/pyoci/issues/217)) ([3737884](https://github.com/AllexVeldman/pyoci/commit/373788445a721bae26cc33d1861c78a40c53042e)), closes [#214](https://github.com/AllexVeldman/pyoci/issues/214)


### Code Refactoring

* **otlp:** Remove `opentelemetry_sdk` and `opentelemetry` deps ([#215](https://github.com/AllexVeldman/pyoci/issues/215)) ([37aca43](https://github.com/AllexVeldman/pyoci/commit/37aca43c182031c6d626523727a0cbd93147b9b2))
* **otlp:** Remove the `otlp` feature flag ([#222](https://github.com/AllexVeldman/pyoci/issues/222)) ([b507bb2](https://github.com/AllexVeldman/pyoci/commit/b507bb20971ee7b70fdf2ebea5cc7a8bc8b5318d))
* **templates:** Replace askama with handlebars ([#223](https://github.com/AllexVeldman/pyoci/issues/223)) ([f4ea36f](https://github.com/AllexVeldman/pyoci/commit/f4ea36f94373ee01ba1cb6db9f6c5fdef793f83a))


### Continuous Integration

* Switch to SonarQube ([#209](https://github.com/AllexVeldman/pyoci/issues/209)) ([c530bf3](https://github.com/AllexVeldman/pyoci/commit/c530bf396c4af14f76b66d66431e84a48c874ab3))


### Dependency Updates

* bump anyhow from 1.0.95 to 1.0.98 ([#204](https://github.com/AllexVeldman/pyoci/issues/204)) ([2aa1141](https://github.com/AllexVeldman/pyoci/commit/2aa114108c142f9de1fee12d42718472356269bd))
* bump async-trait from 0.1.87 to 0.1.88 ([#212](https://github.com/AllexVeldman/pyoci/issues/212)) ([5cc64ff](https://github.com/AllexVeldman/pyoci/commit/5cc64ffbd592c32fe3c11f819b1deeb7bd53bb75))
* bump axum-extra from 0.10.0 to 0.10.1 ([#213](https://github.com/AllexVeldman/pyoci/issues/213)) ([578059b](https://github.com/AllexVeldman/pyoci/commit/578059b0b9812d44a06ab5a071ea8608859e9eb2))
* bump indoc from 2.0.5 to 2.0.6 ([#205](https://github.com/AllexVeldman/pyoci/issues/205)) ([4c3c6e5](https://github.com/AllexVeldman/pyoci/commit/4c3c6e5dc1b8bb80fa89a17a8fd7067ac1cea3bf))
* bump opentelemetry-proto from 0.28.0 to 0.30.0 ([#220](https://github.com/AllexVeldman/pyoci/issues/220)) ([8710508](https://github.com/AllexVeldman/pyoci/commit/8710508dafef29b38de9ed4eff9421a2aaa05b0d))
* bump reqwest from 0.12.12 to 0.12.15 ([#202](https://github.com/AllexVeldman/pyoci/issues/202)) ([7c885d8](https://github.com/AllexVeldman/pyoci/commit/7c885d8e0d3b4973db84d4c1ca6e812a2e0ffc50))
* bump sha2 from 0.10.8 to 0.10.9 ([#218](https://github.com/AllexVeldman/pyoci/issues/218)) ([2ed9f20](https://github.com/AllexVeldman/pyoci/commit/2ed9f2052e5fae6034c7ca82826db1aa8526fe6e))
* bump tokio from 1.44.2 to 1.45.0 ([#211](https://github.com/AllexVeldman/pyoci/issues/211)) ([a28535b](https://github.com/AllexVeldman/pyoci/commit/a28535b1e7646876c4bb7ed1286d658c60a34fa8))
* bump tokio from 1.45.0 to 1.45.1 ([#219](https://github.com/AllexVeldman/pyoci/issues/219)) ([22818a1](https://github.com/AllexVeldman/pyoci/commit/22818a1c8405e1bcf0f0b9da01f72718aa9ef365))
* bump tokio-util from 0.7.14 to 0.7.15 ([#221](https://github.com/AllexVeldman/pyoci/issues/221)) ([4f9ab40](https://github.com/AllexVeldman/pyoci/commit/4f9ab40b3269a68f9cc7a7eaef589605251ae485))

## [0.1.26](https://github.com/AllexVeldman/pyoci/compare/v0.1.25...v0.1.26) (2025-04-10)


### Code Refactoring

* Consolidate unix timestamp nano into the time module ([#200](https://github.com/AllexVeldman/pyoci/issues/200)) ([21c768e](https://github.com/AllexVeldman/pyoci/commit/21c768e7a639fe6d170cfc27c4d62ad65fad8128))
* Move getting UTC time to a separate module ([#199](https://github.com/AllexVeldman/pyoci/issues/199)) ([7c1f9d7](https://github.com/AllexVeldman/pyoci/commit/7c1f9d7b2162483dd8cb47c6a205e96b088c2cab))


### Dependency Updates

* Bump bytes from 1.10.0 to 1.10.1 ([#191](https://github.com/AllexVeldman/pyoci/issues/191)) ([91bdcf9](https://github.com/AllexVeldman/pyoci/commit/91bdcf9596aac8c94c2410c6eef464729da03c6f))
* Bump serde from 1.0.217 to 1.0.219 ([#190](https://github.com/AllexVeldman/pyoci/issues/190)) ([a532878](https://github.com/AllexVeldman/pyoci/commit/a5328789cff840b8fe5ac7bc0c2a12a93c5e0c64))
* bump time from 0.3.37 to 0.3.41 ([#196](https://github.com/AllexVeldman/pyoci/issues/196)) ([a62c0db](https://github.com/AllexVeldman/pyoci/commit/a62c0db6bfa914814c378cd2a226403347517868))
* bump tokio from 1.44.0 to 1.44.2 ([#197](https://github.com/AllexVeldman/pyoci/issues/197)) ([640b65b](https://github.com/AllexVeldman/pyoci/commit/640b65b2cb5a4e44aad5f6aeaafb1c4bcfc280d2))
* Bump tokio-util from 0.7.13 to 0.7.14 ([#188](https://github.com/AllexVeldman/pyoci/issues/188)) ([16b6fac](https://github.com/AllexVeldman/pyoci/commit/16b6fac95be43677ee2891f47c8fafc14805c555))

## [0.1.25](https://github.com/AllexVeldman/pyoci/compare/v0.1.24...v0.1.25) (2025-03-18)


### Bug Fixes

* `project_urls` not in correct location ([#194](https://github.com/AllexVeldman/pyoci/issues/194)) ([40455a0](https://github.com/AllexVeldman/pyoci/commit/40455a0c8f458fcb379573bb96c04acb7336c2fd))

## [0.1.24](https://github.com/AllexVeldman/pyoci/compare/v0.1.23...v0.1.24) (2025-03-17)


### Features

* Include `project_urls` in `<package>/json` ([#192](https://github.com/AllexVeldman/pyoci/issues/192)) ([68e1fed](https://github.com/AllexVeldman/pyoci/commit/68e1fedf50f91ef8db7e5d3150262cc77422d823))

## [0.1.23](https://github.com/AllexVeldman/pyoci/compare/v0.1.22...v0.1.23) (2025-03-11)


### Dependency Updates

* Bump mockito from 1.6.1 to 1.7.0 ([#184](https://github.com/AllexVeldman/pyoci/issues/184)) ([b559ebd](https://github.com/AllexVeldman/pyoci/commit/b559ebd85b4782f03a3094359aa0a49eafa18cb5))
* Bump pin-project from 1.1.9 to 1.1.10 ([#182](https://github.com/AllexVeldman/pyoci/issues/182)) ([4c46766](https://github.com/AllexVeldman/pyoci/commit/4c4676669fda8fcdb53868e2fd033275b557db54))
* Bump ring from 0.17.8 to 0.17.13 in the cargo group ([#180](https://github.com/AllexVeldman/pyoci/issues/180)) ([360a256](https://github.com/AllexVeldman/pyoci/commit/360a256503e04cd9113f47d9c316435c4a2c564c))
* Bump serde_json from 1.0.139 to 1.0.140 ([#183](https://github.com/AllexVeldman/pyoci/issues/183)) ([9d975d3](https://github.com/AllexVeldman/pyoci/commit/9d975d3c9b70643849b45c96b7575748dab25471))
* Bump tokio from 1.43.0 to 1.44.0 ([#181](https://github.com/AllexVeldman/pyoci/issues/181)) ([381555c](https://github.com/AllexVeldman/pyoci/commit/381555c3a95e99ef8a3a4c5edb35afc9ec2809b2))

## [0.1.22](https://github.com/AllexVeldman/pyoci/compare/v0.1.21...v0.1.22) (2025-03-03)


### Bug Fixes

* Breaking changes in Axum ([#154](https://github.com/AllexVeldman/pyoci/issues/154)) ([9372d1b](https://github.com/AllexVeldman/pyoci/commit/9372d1b2eca224249cab8030434bd66b41b94ee7))


### Miscellaneous Chores

* **release:** Give dependency updates their own section in changelog ([#177](https://github.com/AllexVeldman/pyoci/issues/177)) ([e606350](https://github.com/AllexVeldman/pyoci/commit/e606350fe104ddeed13f4fa6826c135baae992fd))
* Switch console log to a more compact format ([#174](https://github.com/AllexVeldman/pyoci/issues/174)) ([7b90e86](https://github.com/AllexVeldman/pyoci/commit/7b90e86cf6756ffa8286c893b1248f041a962f31))


### Dependency Updates

* bump async-trait from 0.1.85 to 0.1.87 ([#176](https://github.com/AllexVeldman/pyoci/issues/176)) ([f1d7eb7](https://github.com/AllexVeldman/pyoci/commit/f1d7eb777acd09164857ae61462ef329db4097fd))
* bump axum from 0.7.9 to 0.8.1 ([#154](https://github.com/AllexVeldman/pyoci/issues/154)) ([9372d1b](https://github.com/AllexVeldman/pyoci/commit/9372d1b2eca224249cab8030434bd66b41b94ee7))
* bump bytes from 1.9.0 to 1.10.0 ([#171](https://github.com/AllexVeldman/pyoci/issues/171)) ([45eb946](https://github.com/AllexVeldman/pyoci/commit/45eb946e1e326cc0e295ffdff3feb69c5eac42dd))
* bump pin-project from 1.1.8 to 1.1.9 ([#172](https://github.com/AllexVeldman/pyoci/issues/172)) ([4921104](https://github.com/AllexVeldman/pyoci/commit/49211044307bd7adc569232c323650cc06610c18))
* bump prost from 0.13.4 to 0.13.5 ([#179](https://github.com/AllexVeldman/pyoci/issues/179)) ([a124bdb](https://github.com/AllexVeldman/pyoci/commit/a124bdb3fea0469418c082b4a927916efac91c73))
* bump serde_json from 1.0.137 to 1.0.139 ([#173](https://github.com/AllexVeldman/pyoci/issues/173)) ([45edf47](https://github.com/AllexVeldman/pyoci/commit/45edf47dc077794e09eb8319c9fcbc84c172390a))
* bump the opentelemetry group with 3 updates ([#178](https://github.com/AllexVeldman/pyoci/issues/178)) ([2913720](https://github.com/AllexVeldman/pyoci/commit/2913720bdc26b0979a0bc961cf5dd3e851db2dde))

## [0.1.21](https://github.com/AllexVeldman/pyoci/compare/v0.1.20...v0.1.21) (2025-02-06)


### Bug Fixes

* Package version list incomplete ([#167](https://github.com/AllexVeldman/pyoci/issues/167)) ([1660245](https://github.com/AllexVeldman/pyoci/commit/1660245b1a0c8c5fc918b8040b40b5241749d56e)), closes [#166](https://github.com/AllexVeldman/pyoci/issues/166)

## [0.1.20](https://github.com/AllexVeldman/pyoci/compare/v0.1.19...v0.1.20) (2025-01-31)


### Features

* Add the package digest to `list_package` ([#163](https://github.com/AllexVeldman/pyoci/issues/163)) ([7fd3ccd](https://github.com/AllexVeldman/pyoci/commit/7fd3ccd4157225611934369e2260399da09bd9ab)), closes [#160](https://github.com/AllexVeldman/pyoci/issues/160)
* Allow setting labels on the OCI image ([#159](https://github.com/AllexVeldman/pyoci/issues/159)) ([6140d20](https://github.com/AllexVeldman/pyoci/commit/6140d200529d80286fb6f8ac347fd75e888cd8ab))
* SHA256 digest of the provided content is checked against the provided `sha256_digest` in the request. [#162](https://github.com/AllexVeldman/pyoci/issues/162) ([968afe8](https://github.com/AllexVeldman/pyoci/commit/968afe89ed540f81fcb8bd0994d2e7938741ab7f))


### Miscellaneous Chores

* **deps:** bump pin-project from 1.1.7 to 1.1.8 ([#157](https://github.com/AllexVeldman/pyoci/issues/157)) ([8d9d0d2](https://github.com/AllexVeldman/pyoci/commit/8d9d0d27e7974e91efce5cc8a3429f891c2b987e))
* **deps:** bump reqwest from 0.12.9 to 0.12.12 ([#156](https://github.com/AllexVeldman/pyoci/issues/156)) ([b0160cb](https://github.com/AllexVeldman/pyoci/commit/b0160cb897145ee2ccee08ee6511f11d49d50361))
* **deps:** bump serde_json from 1.0.135 to 1.0.137 ([#155](https://github.com/AllexVeldman/pyoci/issues/155)) ([ed7f932](https://github.com/AllexVeldman/pyoci/commit/ed7f932484de0cdc2005f9af8d65284ed006b510))


### Code Refactoring

* Refactor of the publish path and HttpTransport [#162](https://github.com/AllexVeldman/pyoci/issues/162) ([968afe8](https://github.com/AllexVeldman/pyoci/commit/968afe89ed540f81fcb8bd0994d2e7938741ab7f))

## [0.1.19](https://github.com/AllexVeldman/pyoci/compare/v0.1.18...v0.1.19) (2025-01-13)


### Features

* Allow configuring the max body size ([#153](https://github.com/AllexVeldman/pyoci/issues/153)) ([852818f](https://github.com/AllexVeldman/pyoci/commit/852818fe8ef4001e37a6e008b21fa06dab378246))


### Bug Fixes

* Return HTTP 413 Payload Too Large instead of 500 ([852818f](https://github.com/AllexVeldman/pyoci/commit/852818fe8ef4001e37a6e008b21fa06dab378246))


### Documentation

* Add CONTRIBUTING and examples README ([#146](https://github.com/AllexVeldman/pyoci/issues/146)) ([7a0d8ab](https://github.com/AllexVeldman/pyoci/commit/7a0d8abef2ee480c3fa140f45830292e58dad48e))


### Miscellaneous Chores

* **deps:** bump async-trait from 0.1.84 to 0.1.85 ([#148](https://github.com/AllexVeldman/pyoci/issues/148)) ([bd0000d](https://github.com/AllexVeldman/pyoci/commit/bd0000d4e674502f183726ab20c7d347db6d8006))
* **deps:** bump bytes from 1.8.0 to 1.9.0 ([#150](https://github.com/AllexVeldman/pyoci/issues/150)) ([5d0b3f7](https://github.com/AllexVeldman/pyoci/commit/5d0b3f755724996af2c25c0724fbb60be9e67a37))
* **deps:** bump serde from 1.0.215 to 1.0.217 ([#152](https://github.com/AllexVeldman/pyoci/issues/152)) ([56fa102](https://github.com/AllexVeldman/pyoci/commit/56fa102d2a2432d12e66d6ad236db78932589ba8))
* **deps:** bump serde_json from 1.0.133 to 1.0.135 ([#149](https://github.com/AllexVeldman/pyoci/issues/149)) ([4022267](https://github.com/AllexVeldman/pyoci/commit/4022267194fbb2cafa30557f156efd9dfde46da2))
* **deps:** bump tokio from 1.41.1 to 1.43.0 ([#145](https://github.com/AllexVeldman/pyoci/issues/145)) ([2984aa8](https://github.com/AllexVeldman/pyoci/commit/2984aa8ee31b3f4cd9f4ab6b9d93e1edb89e56f0))
* **deps:** bump tower from 0.5.1 to 0.5.2 ([#151](https://github.com/AllexVeldman/pyoci/issues/151)) ([beddeb2](https://github.com/AllexVeldman/pyoci/commit/beddeb2f0ba5ed10b23f798b1883850df130a4e8))

## [0.1.18](https://github.com/AllexVeldman/pyoci/compare/v0.1.17...v0.1.18) (2025-01-09)


### Features

* Allow hosting PyOCI on a subpath ([#142](https://github.com/AllexVeldman/pyoci/issues/142)) ([328044e](https://github.com/AllexVeldman/pyoci/commit/328044e9b4bb09af67f2237c1fb8c29dfde172e8)), closes [#141](https://github.com/AllexVeldman/pyoci/issues/141)
* Health endpoint ([#143](https://github.com/AllexVeldman/pyoci/issues/143)) ([090f520](https://github.com/AllexVeldman/pyoci/commit/090f5202886ca4110db7016e531d0f96c71f6452))


### Miscellaneous Chores

* **deps:** bump async-trait from 0.1.83 to 0.1.84 ([#136](https://github.com/AllexVeldman/pyoci/issues/136)) ([e00e5c0](https://github.com/AllexVeldman/pyoci/commit/e00e5c0d2353e522514705f2b6c9e6a5d1b94003))
* **deps:** bump prost from 0.13.3 to 0.13.4 ([#139](https://github.com/AllexVeldman/pyoci/issues/139)) ([75aa552](https://github.com/AllexVeldman/pyoci/commit/75aa552e26b8f0e0abd178f133ed3a2ed0e1a8e9))
* **deps:** bump tokio-util from 0.7.12 to 0.7.13 ([#138](https://github.com/AllexVeldman/pyoci/issues/138)) ([821c3c4](https://github.com/AllexVeldman/pyoci/commit/821c3c427efd29b86b574f28147b8a1929cd4d89))
* **deps:** bump tracing-subscriber from 0.3.18 to 0.3.19 ([#140](https://github.com/AllexVeldman/pyoci/issues/140)) ([5f886e0](https://github.com/AllexVeldman/pyoci/commit/5f886e07a582c1665c94baab6ecaada90dc7a3e8))

## [0.1.17](https://github.com/AllexVeldman/pyoci/compare/v0.1.16...v0.1.17) (2024-12-30)


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.93 to 1.0.95 ([#134](https://github.com/AllexVeldman/pyoci/issues/134)) ([8ade5e2](https://github.com/AllexVeldman/pyoci/commit/8ade5e2a90688ca97560d132295e8c45988decf2))
* **deps:** bump http from 1.1.0 to 1.2.0 ([#131](https://github.com/AllexVeldman/pyoci/issues/131)) ([e1fdcd9](https://github.com/AllexVeldman/pyoci/commit/e1fdcd98ee5d445bb18f8872dd4c8c8fef3f11a2))
* **deps:** bump the opentelemetry group with 2 updates ([#129](https://github.com/AllexVeldman/pyoci/issues/129)) ([4205cc8](https://github.com/AllexVeldman/pyoci/commit/4205cc8be04f8760a12dd053fe6076c8786b4b72))
* **deps:** bump time from 0.3.36 to 0.3.37 ([#132](https://github.com/AllexVeldman/pyoci/issues/132)) ([eaa5ca3](https://github.com/AllexVeldman/pyoci/commit/eaa5ca3f82fb36ff66b6252825a39c6d66b847bc))
* **deps:** bump tracing from 0.1.40 to 0.1.41 ([#130](https://github.com/AllexVeldman/pyoci/issues/130)) ([de3309b](https://github.com/AllexVeldman/pyoci/commit/de3309b0dff96f4c577893a83e28d19afe7fa9d0))

## [0.1.16](https://github.com/AllexVeldman/pyoci/compare/v0.1.15...v0.1.16) (2024-12-07)


### Miscellaneous Chores

* **trace:** Add trace attributes ([#127](https://github.com/AllexVeldman/pyoci/issues/127)) ([7ead818](https://github.com/AllexVeldman/pyoci/commit/7ead818f09f5990969da8b8bfb18834510dc2dc3))

## [0.1.15](https://github.com/AllexVeldman/pyoci/compare/v0.1.14...v0.1.15) (2024-11-25)


### Miscellaneous Chores

* Add created timestamp to manifest ([#123](https://github.com/AllexVeldman/pyoci/issues/123)) ([fec6287](https://github.com/AllexVeldman/pyoci/commit/fec6287df33768263eae13bde6f8c83bc6401048))
* **deps:** bump url from 2.5.3 to 2.5.4 ([#126](https://github.com/AllexVeldman/pyoci/issues/126)) ([3319f9c](https://github.com/AllexVeldman/pyoci/commit/3319f9c61d65a07af794bd9390caef53681d3dcd))
* **tests:** Add test for adding a file to existing index ([#125](https://github.com/AllexVeldman/pyoci/issues/125)) ([40e3c11](https://github.com/AllexVeldman/pyoci/commit/40e3c115aa3747400dece4db0f53eeeae6450be6))

## [0.1.14](https://github.com/AllexVeldman/pyoci/compare/v0.1.13...v0.1.14) (2024-11-20)


### Bug Fixes

* Return 400 BAD_REQUEST for invalid filenames ([#122](https://github.com/AllexVeldman/pyoci/issues/122)) ([48d15c3](https://github.com/AllexVeldman/pyoci/commit/48d15c339227f01796fc6ea8d679088ceffb83ca))


### Miscellaneous Chores

* Include creation date in image index ([#120](https://github.com/AllexVeldman/pyoci/issues/120)) ([37b0b3b](https://github.com/AllexVeldman/pyoci/commit/37b0b3b1cbe1c91782158b076961079172645164))

## [0.1.13](https://github.com/AllexVeldman/pyoci/compare/v0.1.12...v0.1.13) (2024-11-18)


### Documentation

* Add Azure Container Registry to tested registries ([c349a21](https://github.com/AllexVeldman/pyoci/commit/c349a21fe989a535304d91f5407b4790c398c980))
* Update delete section ([#119](https://github.com/AllexVeldman/pyoci/issues/119)) ([a881a11](https://github.com/AllexVeldman/pyoci/commit/a881a11f2cb983238f4a0c427cb943054a5376c2))


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.91 to 1.0.93 ([#112](https://github.com/AllexVeldman/pyoci/issues/112)) ([dce3d0c](https://github.com/AllexVeldman/pyoci/commit/dce3d0cba57dbe41e91c6df04c86ed20a011b8bd))
* **deps:** bump axum from 0.7.7 to 0.7.9 ([#116](https://github.com/AllexVeldman/pyoci/issues/116)) ([3292be3](https://github.com/AllexVeldman/pyoci/commit/3292be3a9aab955957c3d8527e4e50e44c87c1aa))
* **deps:** bump mockito from 1.5.0 to 1.6.1 ([#117](https://github.com/AllexVeldman/pyoci/issues/117)) ([46b3556](https://github.com/AllexVeldman/pyoci/commit/46b35569c97fcf4494bb5aeb3667c04ffdbfa4e6))
* **deps:** bump oci-spec from 0.7.0 to 0.7.1 ([#118](https://github.com/AllexVeldman/pyoci/issues/118)) ([2776be5](https://github.com/AllexVeldman/pyoci/commit/2776be5e1c08f7a948729c05284a9be7e42e289f))
* **deps:** bump reqwest from 0.12.8 to 0.12.9 ([#107](https://github.com/AllexVeldman/pyoci/issues/107)) ([4e063c1](https://github.com/AllexVeldman/pyoci/commit/4e063c18cf64feb7c83cc41c211a741346b49f0b))
* **deps:** bump serde from 1.0.213 to 1.0.215 ([#113](https://github.com/AllexVeldman/pyoci/issues/113)) ([3915668](https://github.com/AllexVeldman/pyoci/commit/39156681378cf33fbf5c437f79f692ffff783ad6))
* **deps:** bump serde_json from 1.0.132 to 1.0.133 ([#115](https://github.com/AllexVeldman/pyoci/issues/115)) ([9092c3b](https://github.com/AllexVeldman/pyoci/commit/9092c3bbe36ed64577769109aa1116f739d7e26d))
* **deps:** bump the opentelemetry group with 3 updates ([#114](https://github.com/AllexVeldman/pyoci/issues/114)) ([e13b424](https://github.com/AllexVeldman/pyoci/commit/e13b42475930e13ceda0ea1c293c4c865f44bd3e))
* **deps:** bump tokio from 1.41.0 to 1.41.1 ([#110](https://github.com/AllexVeldman/pyoci/issues/110)) ([7463838](https://github.com/AllexVeldman/pyoci/commit/74638385da5bb34cb5b547d7c25281c9ca4de876))
* **deps:** bump url from 2.5.2 to 2.5.3 ([#111](https://github.com/AllexVeldman/pyoci/issues/111)) ([f7df494](https://github.com/AllexVeldman/pyoci/commit/f7df4949f2b13538abcdbdd9746c2eaa606108c5))

## [0.1.12](https://github.com/AllexVeldman/pyoci/compare/v0.1.11...v0.1.12) (2024-10-28)


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.90 to 1.0.91 ([#103](https://github.com/AllexVeldman/pyoci/issues/103)) ([d9e06d8](https://github.com/AllexVeldman/pyoci/commit/d9e06d86d401485066f94a1d18b652fe77175f32))
* **deps:** bump bytes from 1.7.2 to 1.8.0 ([#100](https://github.com/AllexVeldman/pyoci/issues/100)) ([a1a2952](https://github.com/AllexVeldman/pyoci/commit/a1a2952986383746fb893ff1e4f90ce97705eae1))
* **deps:** bump pin-project from 1.1.6 to 1.1.7 ([#104](https://github.com/AllexVeldman/pyoci/issues/104)) ([452cbc7](https://github.com/AllexVeldman/pyoci/commit/452cbc79b4bf90d825184385e9bc8055ccc7bb4c))
* **deps:** bump serde from 1.0.210 to 1.0.213 ([#101](https://github.com/AllexVeldman/pyoci/issues/101)) ([f4d918a](https://github.com/AllexVeldman/pyoci/commit/f4d918acc8a892494b9b84a1d9f8515e5444410c))
* **deps:** bump tokio from 1.40.0 to 1.41.0 ([#102](https://github.com/AllexVeldman/pyoci/issues/102)) ([cb607da](https://github.com/AllexVeldman/pyoci/commit/cb607da78b3331e6227be82ee829eaeb986690ca))

## [0.1.11](https://github.com/AllexVeldman/pyoci/compare/v0.1.10...v0.1.11) (2024-10-21)


### Features

* Instruct caches for root and unmatched routes ([#98](https://github.com/AllexVeldman/pyoci/issues/98)) ([f27c3c1](https://github.com/AllexVeldman/pyoci/commit/f27c3c102e660f6546af07fd78acbfa612d743c4))


### Miscellaneous Chores

* **deps:** bump anyhow from 1.0.89 to 1.0.90 ([#96](https://github.com/AllexVeldman/pyoci/issues/96)) ([5b8bcdb](https://github.com/AllexVeldman/pyoci/commit/5b8bcdb6137a172d39407ac51ca013bb3f24a7c4))
* **deps:** bump futures from 0.3.30 to 0.3.31 ([#94](https://github.com/AllexVeldman/pyoci/issues/94)) ([ac5a7d3](https://github.com/AllexVeldman/pyoci/commit/ac5a7d38ac3d42e9da35da47fc101d49eac73aae))
* **deps:** bump pin-project from 1.1.5 to 1.1.6 ([#97](https://github.com/AllexVeldman/pyoci/issues/97)) ([4861163](https://github.com/AllexVeldman/pyoci/commit/486116334812212d7b540f96fdcb2975139707dc))
* **deps:** bump serde_json from 1.0.128 to 1.0.132 ([#95](https://github.com/AllexVeldman/pyoci/issues/95)) ([69deb3c](https://github.com/AllexVeldman/pyoci/commit/69deb3cba16c1a3714a351eaa3138d310b3e7fbf))

## [0.1.10](https://github.com/AllexVeldman/pyoci/compare/v0.1.9...v0.1.10) (2024-10-18)


### Features

* **OTLP:** Add requests count metric ([#92](https://github.com/AllexVeldman/pyoci/issues/92)) ([289d316](https://github.com/AllexVeldman/pyoci/commit/289d316e25bea262db3571dcc1ac74e2560e318c))

## [0.1.9](https://github.com/AllexVeldman/pyoci/compare/v0.1.8...v0.1.9) (2024-10-18)


### Features

* **OTLP:** Collect uptime metrics ([#90](https://github.com/AllexVeldman/pyoci/issues/90)) ([de8a06c](https://github.com/AllexVeldman/pyoci/commit/de8a06c53f3e1a201fdec3b914e77186f9aa4fdb))

## [0.1.8](https://github.com/AllexVeldman/pyoci/compare/v0.1.7...v0.1.8) (2024-10-17)


### Features

* **log:** Include host is logs ([#88](https://github.com/AllexVeldman/pyoci/issues/88)) ([c65c0b6](https://github.com/AllexVeldman/pyoci/commit/c65c0b62dafbddf8e3a862db36a06d8cfbfe8e32))
* **OTLP:** Include version in OTLP logs/traces ([#84](https://github.com/AllexVeldman/pyoci/issues/84)) ([d9d782f](https://github.com/AllexVeldman/pyoci/commit/d9d782fc88410d4bab4136b70bcb006f2475e202))


### Bug Fixes

* **auth:** Log a warning when no auth is provided ([#86](https://github.com/AllexVeldman/pyoci/issues/86)) ([3362f4d](https://github.com/AllexVeldman/pyoci/commit/3362f4d1d3068e8827d8b6920687df109c14a8b5))


### Code Refactoring

* Remove use of Regex ([#87](https://github.com/AllexVeldman/pyoci/issues/87)) ([98708e1](https://github.com/AllexVeldman/pyoci/commit/98708e1357a065d618242530d6a6cd8805fdeaab))


### Build System

* Use Github cache for docker build ([#89](https://github.com/AllexVeldman/pyoci/issues/89)) ([375b810](https://github.com/AllexVeldman/pyoci/commit/375b81059d96afb36914ba4b8628d0eb25803bb2))

## [0.1.7](https://github.com/AllexVeldman/pyoci/compare/v0.1.6...v0.1.7) (2024-10-17)


### Continuous Integration

* **release:** Can't rely on workflow booleans ([ffef246](https://github.com/AllexVeldman/pyoci/commit/ffef2465f4f98c32a237047fe64157e68da8bf24))
* **release:** Fix publish output not being a boolean ([#82](https://github.com/AllexVeldman/pyoci/issues/82)) ([956924d](https://github.com/AllexVeldman/pyoci/commit/956924d9acc69858b6762354ca9c49491c9d3805))

## [0.1.6](https://github.com/AllexVeldman/pyoci/compare/0.1.5...v0.1.6) (2024-10-17)


### Features

* **auth:** Include `scope` in the token exchange ([#75](https://github.com/AllexVeldman/pyoci/issues/75)) ([1a17f48](https://github.com/AllexVeldman/pyoci/commit/1a17f4803eafb78ba1a393864ef5be070b3c872d))


### Documentation

* Add links to specs ([b4a4802](https://github.com/AllexVeldman/pyoci/commit/b4a480274df9e0079e1e69e57efd8ca34e9404fc))


### Build System

* Include license in docker image ([#81](https://github.com/AllexVeldman/pyoci/issues/81)) ([6fb4c1c](https://github.com/AllexVeldman/pyoci/commit/6fb4c1c099eba1797df548337910cd4a97bc4017))


### Continuous Integration

* **release:** Configure release-please ([#74](https://github.com/AllexVeldman/pyoci/issues/74)) ([1c30d98](https://github.com/AllexVeldman/pyoci/commit/1c30d98521c698455a98b6cc0f18cd74287bac80))
