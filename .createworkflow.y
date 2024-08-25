name: Build touchHLE

on:
  push:
    branches: [ "tmp-test" ]
  pull_request:
    branches: [ "trunk" ]

env:
  CARGO_TERM_COLOR: always

jobs:
  build-osx:

    runs-on: macos-latest

    steps:
    - uses: actions/checkout@v3
      with:
        fetch-depth: 0 # touchHLE's git-describe versioning needs tag history
    - name: Install clang-format
      run: brew install clang-format
    - name: Check formatting
      run: dev-scripts/format.sh --check
    - name: Get Submodules
      run: git submodule update --init
    - name: Install Boost
      run: brew install boost
    # This is the earliest point at which we can do linting: clippy will end up
    # building dynarmic, which needs Boost, but we don't need LLVM until
    # building tests.
    - name: Lint
      run: dev-scripts/lint.sh
    - name: Try to get cached copy of LLVM
      id: cache-llvm
      uses: actions/cache@v3
      with:
        path: tests/llvm
        key: llvm_12_0_0_macOS_x64
    - if: ${{ steps.cache-llvm.outputs.cache-hit != 'true' }}
      name: Download LLVM
      run: curl -L -O "https://github.com/llvm/llvm-project/releases/download/llvmorg-12.0.0/clang+llvm-12.0.0-x86_64-apple-darwin.tar.xz"
    - if: ${{ steps.cache-llvm.outputs.cache-hit != 'true' }}
      name: Extract LLVM
      run: tar -xf clang+llvm-12.0.0-x86_64-apple-darwin.tar.xz && mkdir tests/llvm && mv clang+llvm-12.0.0-x86_64-apple-darwin/* tests/llvm
    - name: Test
      run: cargo test
    - name: Build
      run: cargo build --release && mv target/release/touchHLE .
    - uses: actions/upload-artifact@v3
      with:
        name: touchHLE_macOS_x86_64
        path: touchHLE
    - uses: actions/upload-artifact@v3
      with:
        name: TestApp_built_on_macOS
        path: tests/TestApp.app

  build-android:

    runs-on: ubuntu-latest

    steps:
    - uses: actions/checkout@v3
      with:
        fetch-depth: 0 # touchHLE's git-describe versioning needs tag history
    - name: Get Submodules
      run: git submodule update --init
    - name: Try to get cached copy of Boost
      id: cache-boost
      uses: actions/cache@v3
      with:
        path: vendor/boost
        key: boost_1_81_0
    - if: ${{ steps.cache-boost.outputs.cache-hit != 'true' }}
      name: Download Boost
      run: curl -L -o boost_1_81_0.7z "https://boostorg.jfrog.io/artifactory/main/release/1.81.0/source/boost_1_81_0.7z"
    - if: ${{ steps.cache-boost.outputs.cache-hit != 'true' }}
      name: Extract Boost
      run: 7z -ovendor x boost_1_81_0.7z && mv vendor/boost_1_81_0 vendor/boost
    - name: Install AArch64 Android Rust toolchain
      run: rustup target add aarch64-linux-android
    - name: Install cargo-ndk
      run: cargo install cargo-ndk
    # Gradle version compatibility is a nightmare and we refuse to use gradlew.
    - name: Try to get a cached copy of Gradle
      id: cache-gradle
      uses: actions/cache@v3
      with:
        path: gradle
        key: gradle-7.3
    - if: ${{ steps.cache-gradle.outputs.cache-hit != 'true' }}
      name: Download Gradle
      run: curl -L -o gradle-7.3-bin.zip "https://services.gradle.org/distributions/gradle-7.3-bin.zip"
    - if: ${{ steps.cache-gradle.outputs.cache-hit != 'true' }}
      name: Extract Gradle
      run: 7z -ogradle x gradle-7.3-bin.zip
    # Why isn't there a command for this?
    - name: Generate local.properties file
      run: echo 'sdk.dir='$ANDROID_SDK_ROOT > android/local.properties
    # Let's not even bother with linting (would need to build for host, not just
    # Android, which makes things more complicated) or trying to run tests
    # (would have to run on Android 😰).
    - name: Build for Android
      run: cd android && ../gradle/gradle-7.3/bin/gradle assembleRelease && mv app/build/outputs/apk/release/app-release.apk ../touchHLE.apk
    - uses: actions/upload-artifact@v3
      with:
        name: touchHLE_Android_AArch64
        path: touchHLE.apk

  build-win:

    runs-on: windows-latest

    steps:
    - uses: actions/checkout@v3
      with:
        fetch-depth: 0 # touchHLE's git-describe versioning needs tag history
    # Apparently the Windows runner already has clang-format? That's nice.
    - name: Check formatting
      shell: bash # no command output without this
      run: dev-scripts/format.sh --check
    - name: Get Submodules
      run: git submodule update --init
    - name: Try to get cached copy of Boost
      id: cache-boost
      uses: actions/cache@v3
      with:
        path: vendor\boost
        key: boost_1_81_0
    - if: ${{ steps.cache-boost.outputs.cache-hit != 'true' }}
      name: Download Boost
      run: curl -L -o boost_1_81_0.7z "https://boostorg.jfrog.io/artifactory/main/release/1.81.0/source/boost_1_81_0.7z"
    - if: ${{ steps.cache-boost.outputs.cache-hit != 'true' }}
      name: Extract Boost
      run: 7z -ovendor x boost_1_81_0.7z && ren vendor\boost_1_81_0 boost
    # This is the earliest point at which we can do linting: clippy will end up
    # building dynarmic, which needs Boost, but we don't need LLVM until
    # building tests.
    - name: Lint
      shell: bash # no command output without this
      run: dev-scripts/lint.sh
    - name: Try to get cached copy of LLVM
      id: cache-llvm
      uses: actions/cache@v3
      with:
        path: tests/llvm
        key: llvm_12_0_1_Windows_x64
    - if: ${{ steps.cache-llvm.outputs.cache-hit != 'true' }}
      name: Download LLVM
      run: curl -L -O "https://github.com/llvm/llvm-project/releases/download/llvmorg-12.0.1/LLVM-12.0.1-win64.exe"
    - if: ${{ steps.cache-llvm.outputs.cache-hit != 'true' }}
      name: Extract LLVM
      run: 7z -otests/llvm x LLVM-12.0.1-win64.exe
    - name: Test
      run: cargo test
    - name: Build
      run: cargo build --release && move target/release/touchHLE.exe .
    - uses: actions/upload-artifact@v3
      with:
        name: touchHLE_Windows_x86_64
        path: touchHLE.exe
    - uses: actions/upload-artifact@v3
      with:
        name: TestApp_built_on_Windows
        path: tests/TestApp.app
