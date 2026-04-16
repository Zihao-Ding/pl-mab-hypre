cmake_minimum_required(VERSION 3.5)
project(satsuma)

include(ExternalProject)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(DEJAVU_LOCATION ${CMAKE_SOURCE_DIR}/dejavu)

add_subdirectory(dejavu)

add_compile_options("-O3")
add_compile_options("-Wall")
add_compile_options("-march=native")
# add_definitions(-g)
add_definitions(-DNDEBUG)

add_executable(satsuma satsuma.cpp cvec.cpp hashmap.cpp)