#![feature(globs)]
#![allow(non_snake_case)]

extern crate getopts;
extern crate jni;
extern crate libc;
extern crate serialize;

use getopts::{optflag, getopts, OptGroup};
use jni::*;
use jni::classpath::load_static_method;
use libc::*;
use serialize::json;
use std::io::File;
use std::os;

#[link(name = "packrnative", kind = "static")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "CoreServices", kind = "framework")]
extern {
    fn cfRunLoopRun(callback: extern fn(&Receiver<c_int>), signal:&Receiver<c_int>);
    fn cfRunLoopStop();
}

#[deriving(Decodable)]
struct Config {
    jar: String,
    mainClass: String,
    vmArgs: Vec<String>
}

fn print_usage(program: &str, _opts: &[OptGroup]) {
    println!("Usage: {} [options]", program);
    println!("-h --help\tUsage");
    println!("-- {{args}}\tUnparsed arguments passed through to Java main() method.");
}

fn read_config(path: &Path) -> Config {
    let content = File::open(path).read_to_string().unwrap();
    let config:Config = json::decode(content.as_slice()).unwrap();

    println!("jar: {:s}", config.jar);
    println!("main class: {:s}", config.mainClass);

    for arg in config.vmArgs.iter() {
        println!("VM argument: {}", arg);
    }

    config
}

fn init_jvm_arguments(jni:&mut JNI, config: &Config) {
    let num_args = config.vmArgs.len();
    
    let cp_path = os::make_absolute(&Path::new(config.jar.as_slice()));
    let class_path = format!("-Djava.class.path={}", cp_path.display());

    println!("class path: {}", class_path);

    jni.init_vm_args(num_args + 1u);
    jni.push_vm_arg(0u, class_path.as_slice());

    for i in range(0u, num_args) {
        let ref vm_arg = config.vmArgs[i];
        jni.push_vm_arg(i + 1, vm_arg.as_slice());
    }
}

#[cfg(target_os = "macos")]
fn get_libjvm_path(jre: Path) -> Path {
    let mut path = jre.clone();
    path.push("lib/server/libjvm.dylib");
    path
}

#[cfg(target_os = "linux")]
fn get_libjvm_path(jre: Path) -> Path {
    let mut path = jre.clone();
    path.push("lib/amd64/server/libjvm.so");
    path
}

fn check_for_exceptions(jni:&JNI) {
    let throwable = jni.exception_occured();
    if !JNI::is_null(throwable) {
        jni.exception_describe();
        jni.exception_clear();
        panic!("Exception caught!");
    }
}

fn load_jvm(jni:&mut JNI, config:&Config) {
    // create & fill arguments
    init_jvm_arguments(jni, config);

    // load lib, create VM instance
    match jni.load_jvm() {
        Err(err) => panic!(err),
        Ok(_) => {}
    };

    // attach to current thread
    /*if jni.attach_current_thread() != JNI_OK {
        panic!("Could not attach JVM to thread");
    }
    println!("JVM attached to thread ...");*/
}

fn call_main(jni:&JNI, path_to_jar:&str, main_class_name:&str, args:&Vec<String>) {

    // do class-loader voodoo

    let (main_class, main_method) = load_static_method(jni, path_to_jar, main_class_name);

    // pass program arguments

    let java_lang_String:Jclass = jni.find_class("java/lang/String");
    check_for_exceptions(jni);
    assert!(!JNI::is_null(java_lang_String));

    let argc = args.len();

    let argv = jni.new_object_array(argc as Jint, java_lang_String, JNI_NULL);
    check_for_exceptions(jni);
    assert!(!JNI::is_null(argv));

    for i in range(0u, argc) {
        let arg = jni.new_string_utf(args[i].as_slice());
        jni.set_object_array_element(argv, i as Jint, arg);
    }

    // call main()

    match (main_class, main_method) {
        (JNI_NULL, JNI_NULL) => println!("Could not find {} in {}", main_method, main_class),
        (_, _) => jni.call_static_void_method_a(main_class, main_method, [argv])
    };

    println!("Quit from JVM ...");
}

fn destroy_vm(jni:&JNI) {
    println!("Unloading JVM ...");
    jni.destroy_jvm();
}

fn spawn_vm() {

    let args:Vec<String> = os::args();
    let program = args[0].clone();

    let opts = [
        optflag("h", "help", "print this help menu")
    ];

    let matches = match getopts(args.tail(), opts) {
        Err(f) => {
            println!("{} Run {} --help to show options.", f.to_string(), program);
            return;
        }
        Ok(m) => { m }
    };

    if matches.opt_present("h") {
        print_usage(program.as_slice(), opts);
        return;
    }

    let config_path = os::make_absolute(&Path::new("config.json"));
    println!("config path: {}", config_path.display());

    let root_path = config_path.dir_path();
    println!("pwd: {}", root_path.display());

    // change working dir (MacOS: starts at parent folder of .app)
    if !os::change_dir(&root_path) {
        panic!("Could not change working directory");
    }

    let libjvmpath = get_libjvm_path(os::make_absolute(&Path::new("jre")));
    println!("JRE path: {}", libjvmpath.display());

    // read config JSON
    let config = read_config(&config_path);

    let cp_path = os::make_absolute(&Path::new(config.jar.as_slice()));
    let class_path = format!("file://{}", cp_path.display());

    println!("Loading JVM library ...");
    let mut jni:JNI = match JNI::new(&libjvmpath) {
        Err(error) => panic!(error),
        Ok(jni) => jni
    };

    println!("Creating JVM instance ...");
    load_jvm(&mut jni, &config);

    println!("Invoking {:s}.main()", config.mainClass);
    call_main(&jni, class_path.as_slice(), config.mainClass.as_slice(), &matches.free);

    destroy_vm(&jni);
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

    let (tx, rx): (Sender<c_int>, Receiver<c_int>) = std::comm::channel();
    let proc_tx = tx.clone();

    spawn(proc() {
        spawn_vm();
        proc_tx.send(0);
    });

    unsafe {
        cfRunLoopRun(run_loop_callback, &rx);
    }
}
