#![allow(non_snake_case)]

extern crate getopts;
extern crate jni;
extern crate rustc_serialize;

#[cfg(target_os = "macos")]
extern crate libc;

use getopts::Options;
use jni::JNI;
use jni::classpath::load_static_method;
use jni::consts::*;
use jni::ffi;
use jni::ffi::JNIEnv;
use jni::types as jni_types;
use jni::types::{Jclass, Jint};
use rustc_serialize::json;
use std::env;
use std::io::prelude::*;
use std::fs::File;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use libc::{c_int};
#[cfg(target_os = "macos")]
use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(target_os = "macos")]
use std::thread;

#[derive(RustcDecodable)]
struct Config {
    jar: String,
    mainClass: String,
    vmArgs: Vec<String>
}

#[cfg(target_os = "macos")]
fn select_root_path(program: &str) {
    let program_path = Path::new(&program).parent().unwrap();
    println!("executable: {}", program_path.display());

    if env::set_current_dir(&program_path).is_err() {
        panic!("Could not change working directory");
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(unused_variables)]
fn select_root_path(program: &str) {
}

#[cfg(target_os = "macos")]
fn get_libjvm_path_os(path: &mut PathBuf) {
    path.push("lib");
    path.push("jli");
    path.push("libjli");
    path.set_extension("dylib");
}

#[cfg(all(target_os = "linux", arch = "x86_64"))]
fn get_libjvm_path_os(path: &mut PathBuf) {
    path.push("lib");
    path.push("amd64");
    path.push("server");
    path.push("libjvm");
    path.set_extension("so");
}

#[cfg(all(target_os = "linux", arch = "x86"))]
fn get_libjvm_path_os(path: &mut PathBuf) {
    path.push("lib");
    path.push("i386");
    path.push("server");
    path.push("libjvm");
    path.set_extension("so");
}

#[cfg(target_os = "windows")]
fn get_libjvm_path_os(path: &mut PathBuf) {
    path.push("bin");
    path.push("server");
    path.push("jvm");
    path.set_extension("dll");
}

fn get_libjvm_path(jre: &Path) -> PathBuf {
    let mut path = PathBuf::new();
    path.push(jre);
    get_libjvm_path_os(&mut path);
    path
}

fn init_jvm_arguments(jni: &mut JNI, config: &Config) {
    let num_args = config.vmArgs.len();
    
    jni.init_vm_args(num_args);

    for i in 0..num_args {
        let ref vm_arg = config.vmArgs[i];
        jni.push_vm_arg(i, &vm_arg);
    }
}

fn load_jvm(jni: &mut JNI, config: &Config) {
    // create & fill arguments
    init_jvm_arguments(jni, config);

    // load lib, create VM instance
    match jni.create_java_vm() {
        Err(err) => panic!(err),
        Ok(_) => {}
    };

    // attach to current thread
    match ffi::attach_current_thread(jni.get_jvm()) {
        (JNI_OK, _) => {},
        (_, _) => panic!("Could not attach JVM to thread")
    }
    println!("JVM attached to thread ...");
}

fn check_for_exceptions(env: &mut JNIEnv) {
    let throwable = ffi::exception_occured(env);
    if !jni_types::is_null(throwable) {
        ffi::exception_describe(env);
        ffi::exception_clear(env);
        panic!("Exception caught!");
    }
}

fn call_main(env: &mut JNIEnv, path_to_jar: &str, main_class_name: &str, args: &Vec<String>) {

    // do class-loader voodoo

    let (main_class, main_method) = load_static_method(env, path_to_jar, main_class_name);

    match (main_class, main_method) {
        (JNI_NULL, JNI_NULL) => {
            println!("Could not find {}.main()", main_class_name);
            return;
        },
        (_, _) => {}
    };

    // pass program arguments

    let java_lang_String:Jclass = ffi::find_class(env, "java/lang/String");
    check_for_exceptions(env);
    assert!(!jni_types::is_null(java_lang_String));

    let argc = args.len();

    let argv = ffi::new_object_array(env, argc as Jint, java_lang_String, JNI_NULL);
    check_for_exceptions(env);
    assert!(!jni_types::is_null(argv));

    for i in 0..argc {
        println!("Application argument: {}", &args[i]);
        let arg = ffi::new_string_utf(env, &args[i]);
        ffi::set_object_array_element(env, argv, i as Jint, arg);
    }

    // call main()
    ffi::call_static_void_method_a(env, main_class, main_method, &[argv]);

    check_for_exceptions(env);

    println!("Quit from JVM ...");
}

fn read_config(path: &Path) -> Config {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(_) => panic!("Could not open {}", path.display())
    };
    
    let mut content = String::new();
    
    if f.read_to_string(&mut content).is_err() {
        panic!("Error reading {}", path.display());
    }

    let config:Config = json::decode(&content).unwrap();

    println!("jar: {}", config.jar);
    println!("main class: {}", config.mainClass);

    for arg in config.vmArgs.iter() {
        println!("VM argument: {}", arg);
    }

    config
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage:\n    {} [options] [--] [<args>...]", program);
    print!("{}", opts.usage(&brief));
    println!("\n    Trailing arguments are passed to Java main() method.");
}

fn spawn_vm() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => {
            println!("{} Run {} --help to show options.", f.to_string(), program);
            return;
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }

    select_root_path(&program);

    let root_path = env::current_dir().unwrap();
    println!("working directory: {}", root_path.display());

    let libjvmpath = get_libjvm_path(root_path.join("jre").as_path());
    println!("JRE path: {}", libjvmpath.display());

    // read config JSON
    let config = read_config(root_path.join("config.json").as_path());

    let cp_path = Path::new(&config.jar);
    let class_path = format!("{}", cp_path.display());
    println!("class path: {}", class_path);

    println!("Loading JVM library ...");
    let mut jni:JNI = match JNI::new(&libjvmpath) {
        Err(error) => panic!(error),
        Ok(jni) => jni
    };

    println!("JNI initialized.");

    println!("Creating JVM instance ...");
    load_jvm(&mut jni, &config);

    println!("Invoking {}.main()", config.mainClass);
    let env:&mut JNIEnv = jni.get_env();
    call_main(env, &class_path, &config.mainClass, &matches.free);
}

#[cfg(target_os = "macos")]
#[link(name = "packrnative", kind = "static")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "CoreServices", kind = "framework")]
extern {
    fn cfRunLoopRun(callback: extern fn(&Receiver<c_int>), signal:&Receiver<c_int>);
    fn cfRunLoopStop();
}

#[cfg(target_os = "macos")]
extern fn run_loop_callback(signal:&Receiver<c_int>) {
    match signal.try_recv() {
        Err(_) => {},
        Ok(_) => unsafe {
            cfRunLoopStop();
        } 
    }
}

#[cfg(target_os = "macos")]
fn main() {

    let (tx, rx): (Sender<c_int>, Receiver<c_int>) = channel();
    let proc_tx = tx.clone();

    thread::spawn(move|| {
        spawn_vm();
        proc_tx.send(0).unwrap();
    });

    unsafe {
        cfRunLoopRun(run_loop_callback, &rx);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    spawn_vm();
}
