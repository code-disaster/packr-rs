#![feature(globs)]
#![allow(non_snake_case)]

extern crate getopts;
extern crate jni;
extern crate libc;
extern crate serialize;

use getopts::{optopt, optflag, getopts, OptGroup};
use jni::*;
use libc::*;
use serialize::json;
use std::io::File;
use std::os;

#[deriving(Decodable)]
struct Config {
	jar: String,
	mainClass: String,
	vmArgs: Vec<String>
}

fn print_usage(program: &str, _opts: &[OptGroup]) {
    println!("Usage: {} [options]", program);
    println!("-h --help\tUsage");
}

fn read_config(path: &Path) -> Config {
	let content = File::open(path).read_to_string().unwrap();
	let config:Config = json::decode(content.as_slice()).unwrap();

	println!("jar: {:s}", config.jar);
	println!("main class: {:s}", config.mainClass);

	for arg in config.vmArgs.iter() {
		println!("vm argument: {}", arg);
	}

	config
}

fn init_jvm_arguments(config: &Config) -> JavaVMInitArgs {
	let num_args = 1u + config.vmArgs.len();
	
	let cp_path = os::make_absolute(&Path::new(config.jar.as_slice()));
	let class_path = format!("-Djava.class.path={}", cp_path.display());

	println!("class path: {}", class_path);

	let mut vm_args = match JavaVMInitArgs::new(num_args) {
		Err(err) => fail!("Could not create VM init arguments: {}", err),
		Ok(vm_args) => vm_args
	};
	
	for i in range(0u, num_args - 1u) {
		let ref vm_arg = config.vmArgs[i];
		vm_args.push(i, vm_arg);
	}

	vm_args.push(num_args - 1u, &class_path);

	vm_args
}

#[cfg(target_os = "macos")]
fn get_libjvm_path(jre: Path) -> Path {
	let mut path = jre.clone();
	path.push("lib/server/libjvm.dylib");
	path
}

fn load_jvm(jni:&mut JNI, config:&Config) {
	// create & fill arguments
	let vm_args = init_jvm_arguments(config);

	// load lib, create VM instance
	match jni.load_jvm(&vm_args) {
		Err(err) => fail!(err),
		Ok(_) => {}
	};

	// attach to current thread
    if jni.attach_current_thread() != JNI_OK {
    	fail!("Could not attach JVM to thread");
    }
	println!("JVM attached to thread ...");

	let version:Jint = jni.get_version();
	println!("JNIEnv get_version(): {:x}", version);
}

fn call_main(jni:&JNI, main_class_name:&String/*, args:&Vec<String>*/) {
	let java_lang_String:Jclass = jni.find_class("java/lang/String");
	assert!(!jni_pointer_is_null(java_lang_String));

   	// find main class
   	let ref main_class_str = main_class_name.as_slice();
   	let main_class:Jclass = jni.find_class(*main_class_str);

   	match jni_pointer_is_null(main_class) {
   		true => fail!("Could not find main class {:s}", *main_class_str),
   		false => {}
   	}

   	//let main_method_id = jni.get_static_method_id(&main_class, "main", "(Ljava/lang/String;)V");
}

fn destroy_vm(jni:&JNI) {
    println!("Unloading JVM ...");
    jni.destroy_jvm();
}

fn main() {

    let args:Vec<String> = os::args();
    let program = args[0].clone();

    let opts = [
		optflag("h", "help", "print this help menu")
    ];

	let matches = match getopts(args.tail(), opts) {
        Err(f) => { fail!(f.to_string()) }
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

	// check: do we need os.change_dir?
	if !os::change_dir(&root_path) {
		fail!("Could not change working directory");
	}

    let libjvmpath = get_libjvm_path(os::make_absolute(&Path::new("jre")));
    println!("JRE path: {}", libjvmpath.display());

    // read config JSON
    let config = read_config(&config_path);

	println!("Loading JVM library ...");
    let mut jni:JNI = match JNI::new(&libjvmpath) {
    	Err(error) => fail!(error),
    	Ok(jni) => jni
    };

	println!("Creating JVM instance ...");
    load_jvm(&mut jni, &config);

    println!("Invoking {:s}.main()", config.mainClass);
    call_main(&jni, &config.mainClass);

    destroy_vm(&jni);

    println!("Bye!")
}
