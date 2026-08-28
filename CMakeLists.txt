cmake_minimum_required(VERSION 3.16)
project(EsotereelGui)

# clangdがプロジェクト全体を解析できるようにビルドフラグを出力
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
set(QT_NO_PRIVATE_MODULE_WARNING ON CACHE BOOL "" FORCE)

# cの設定
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTORCC ON)

# Allow setting Qt6 path via environment variable
if(NOT Qt6_DIR)
    if(DEFINED ENV{Qt6_DIR})
        list(APPEND CMAKE_PREFIX_PATH "$ENV{Qt6_DIR}")
    elseif(DEFINED ENV{QT_DIR})
        list(APPEND CMAKE_PREFIX_PATH "$ENV{QT_DIR}")
    endif()
endif()

# ビルドタイプに応じた設定の切り替え
if(NOT CMAKE_BUILD_TYPE OR CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(IS_DEBUG TRUE)
else()
    set(IS_DEBUG FALSE)
endif()

if(IS_DEBUG)
    set(RUST_PROFILE "debug")
    set(RUST_FLAG "")
else()
    set(RUST_PROFILE "release")
    set(RUST_FLAG "--release")
endif()

# ライブラリの拡張子とか
if(WIN32)
    set(DLIB_EXT "dll")
    set(RLIB_PREFIX "")
    set(CLANGD_OS "windows")
    set(CLANGD_DEFINES "    \"-DUNICODE\",\n    \"-D_UNICODE\",")
    set(CLANGD_SYS_INC "")
elseif(APPLE)
    set(DLIB_EXT "dylib")
    set(RLIB_PREFIX "lib")
    set(CLANGD_OS "darwin")
    set(CLANGD_DEFINES "")
    set(CLANGD_SYS_INC "")
else()
    set(DLIB_EXT "so")
    set(RLIB_PREFIX "lib")
    set(CLANGD_OS "linux")
    set(CLANGD_DEFINES "")
    set(CLANGD_SYS_INC "    \"-isystem\", \"/usr/include\",")
endif()

# Qt6 components - Wayland is platform-specific
set(QT_COMPONENTS Widgets Network Gui Core LinguistTools)
if(UNIX AND NOT APPLE)
    list(APPEND QT_COMPONENTS WaylandClient)
endif()

find_package(Qt6 REQUIRED COMPONENTS ${QT_COMPONENTS})

# qt ads
include(FetchContent)
FetchContent_Declare(
    ads
    GIT_REPOSITORY https://github.com/githubuser0xFFFF/Qt-Advanced-Docking-System.git
    GIT_TAG        5.0.0
)
set(BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)  
set(ADS_VERSION "5.0.0" CACHE STRING "" FORCE)  
FetchContent_MakeAvailable(ads)

# ADSターゲットが存在する場合、そのターゲットに対する警告をすべて無効化する
if (TARGET qtadvanceddocking-qt6)
    if(NOT MSVC)
        # GCC/Clang用: すべての警告を抑制 (-w)
        target_compile_options(qtadvanceddocking-qt6 PRIVATE "-w")
    else()
        # MSVC用
        target_compile_options(qtadvanceddocking-qt6 PRIVATE "/W0")
    endif()
endif()

# コンパイラパスの指定
set(CLANGD_COMPILER "${CMAKE_CXX_COMPILER}")

# Qt6のインクルードパスを、clangdが読めるフォーマットに一括変換
set(CLANGD_QT_PATHS "")
if(TARGET Qt6::Core)
    get_target_property(QT6_INC_DIRS Qt6::Core INTERFACE_INCLUDE_DIRECTORIES)
    foreach(dir ${QT6_INC_DIRS})
        # セミコロンやリストのバグを防ぐため、1つずつ安全に整形して蓄積
        set(CLANGD_QT_PATHS "${CLANGD_QT_PATHS}    \"-I${dir}\",\n")
    endforeach()
endif()

# ライブラリとコア
set(RUST_TARGET_DIR "${CMAKE_SOURCE_DIR}/target/${RUST_PROFILE}")
set(GUI_HLP_LIB "${RUST_TARGET_DIR}/${RLIB_PREFIX}esotereel_gui_helper.${DLIB_EXT}")
set(CORE_BIN "${RUST_TARGET_DIR}/esotereel_core")
file(GLOB_RECURSE RUST_SRC_FILES
    "${CMAKE_SOURCE_DIR}/core/src/*"
    "${CMAKE_SOURCE_DIR}/lib/src/*"
    "${CMAKE_SOURCE_DIR}/guihlp/src/*"
    "${CMAKE_SOURCE_DIR}/Cargo.toml"
)

# rustビルドするやつ
set(RUST_BUILD_ENV)

# GUI Helper (ライブラリ) のビルドコマンド
add_custom_command(
    OUTPUT ${GUI_HLP_LIB}
    COMMAND ${CMAKE_COMMAND} -E env ${RUST_BUILD_ENV} cargo build ${RUST_FLAG} -p esotereel_gui_helper
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    DEPENDS ${RUST_SRC_FILES}
    USES_TERMINAL 
    COMMENT "Building Rust GUI Helper (${RUST_PROFILE})..."
)

# Core (サーバー) のビルドコマンド
add_custom_command(
    OUTPUT ${CORE_BIN}
    COMMAND ${CMAKE_COMMAND} -E env ${RUST_BUILD_ENV} cargo build ${RUST_FLAG} -p esotereel_core
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    DEPENDS ${RUST_SRC_FILES}
    USES_TERMINAL 
    COMMENT "Building Rust Core Server (${RUST_PROFILE})..."
)

# GUI実行に必要なヘルパーライブラリのみを自動ビルド対象にする
add_custom_target(rust_gui_helper ALL DEPENDS ${GUI_HLP_LIB})

# サーバー側は必要なときだけ単体でビルドできるように分離 (ALL を外す)
add_custom_target(rust_core DEPENDS ${CORE_BIN})


# Clangのエラー回避
string(REPLACE "-mno-direct-extern-access" "" CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS}")

# 実行ファイル
file(GLOB_RECURSE SRC_FILES CONFIGURE_DEPENDS gui/src/*.cpp)
add_executable(esotereel_gui ${SRC_FILES})

# dll deploy
if(WIN32)
    if(IS_DEBUG)
        set(DLL_DEPLOY_CMD "powershell -ExecutionPolicy Bypass -File ./deploydebug.ps1")
    else()
        set(DLL_DEPLOY_CMD "powershell -ExecutionPolicy Bypass -File ./deploydist.ps1")
    endif()

    add_custom_command(TARGET esotereel_gui POST_BUILD
        COMMAND ${DLL_DEPLOY_CMD}
        WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
    )
endif()

# # ポート使ってるやつ消す
# if(UNIX AND NOT APPLE)
#     add_custom_target(cleanup_port COMMAND fuser -k 12345/tcp || true)
# else()
#     add_custom_target(cleanup_port)
# endif()

# # 依存関係
# add_dependencies(esotereel_gui rust_workspace cleanup_port)

# ライブラリリンク
target_include_directories(esotereel_gui PRIVATE "${CMAKE_SOURCE_DIR}/guihlp/include" "${CMAKE_SOURCE_DIR}/gui/src")
target_link_libraries(esotereel_gui PRIVATE 
    Qt6::Widgets 
    Qt6::Network
    qtadvanceddocking-qt6 
    ${GUI_HLP_LIB}
)

if(TARGET Qt6::WaylandClient)
 # Qt6.10以降のために、Privateモジュールを明示的に探索させる
    find_package(Qt6WaylandClientPrivate REQUIRED)
    
    target_link_libraries(esotereel_gui PRIVATE Qt6::WaylandClient Qt6::WaylandClientPrivate)
endif()

if (CMAKE_BUILD_TYPE STREQUAL "Debug")
    target_compile_options(esotereel_gui PRIVATE -fsanitize=address -fno-omit-frame-pointer)
    target_link_options(esotereel_gui PRIVATE -fsanitize=address)
endif()

# translate 
qt_add_translations(esotereel_gui
    TS_FILES
        esotereel_ja.ts  
        esotereel_en.ts  
)

# .clangd ファイルの動的生成設定
set(CLANGD_REMOVE_FLAG "  Remove: [-mno-direct-extern-access]\n")
set(CLANGD_CONTENT
"If:
  OS: ${CLANGD_OS}
CompileFlags:
  CompilationDatabase: build
  Compiler: ${CLANGD_COMPILER}
${CLANGD_REMOVE_FLAG}
  Add: [
    \"-Igui\",
    \"-Iguihlp/include\",
    \"-Ibuild/_deps/ads-src/src\",
${CLANGD_QT_PATHS}${CLANGD_SYS_INC}
${CLANGD_DEFINES}
    \"-std=c++17\"
  ]
Index:
  Background: Build
"
)

file(WRITE "${CMAKE_CURRENT_SOURCE_DIR}/.clangd" "${CLANGD_CONTENT}")