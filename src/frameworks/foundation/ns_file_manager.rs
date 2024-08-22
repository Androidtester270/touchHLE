/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSFileManager` etc.

use std::io::{Seek, SeekFrom};

use super::{ns_array, ns_string, NSInteger, NSUInteger};
use crate::dyld::{export_c_func, FunctionExports};
use crate::frameworks::foundation::ns_dictionary::dict_from_keys_and_objects;
use crate::fs::{GuestPath, GuestPathBuf};
use crate::mem::MutPtr;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, ClassExports, HostObject,
};
use crate::Environment;

type NSSearchPathDirectory = NSUInteger;
const NSApplicationDirectory: NSSearchPathDirectory = 1;
const NSDocumentDirectory: NSSearchPathDirectory = 9;
const NSApplicationSupportDirectory: NSSearchPathDirectory = 14;

type NSSearchPathDomainMask = NSUInteger;
const NSUserDomainMask: NSSearchPathDomainMask = 1;

fn NSSearchPathForDirectoriesInDomains(
    env: &mut Environment,
    directory: NSSearchPathDirectory,
    domain_mask: NSSearchPathDomainMask,
    expand_tilde: bool,
) -> id {
    // TODO: other cases not implemented
    assert!(domain_mask == NSUserDomainMask);
    assert!(expand_tilde);

    let dir = match directory {
        // This might not actually be correct. I haven't bothered to test it
        // because I can't think of a good reason an iPhone OS app would have to
        // request this; Wolfenstein 3D requests it but never uses it.
        NSApplicationDirectory => GuestPath::new(crate::fs::APPLICATIONS).to_owned(),
        NSDocumentDirectory | 5 => env.fs.home_directory().join("Documents"),
        // NSLibraryDirectory ??
        5 => env.fs.home_directory().join("Documents"),
        NSApplicationSupportDirectory => env
            .fs
            .home_directory()
            .join("Library")
            .join("Application Support"),
        _ => todo!("NSSearchPathDirectory {}", directory),
    };
    let dir = ns_string::from_rust_string(env, String::from(dir));
    let dir_list = ns_array::from_vec(env, vec![dir]);
    autorelease(env, dir_list)
}

fn NSHomeDirectory(env: &mut Environment) -> id {
    let dir = env.fs.home_directory();
    let dir = ns_string::from_rust_string(env, String::from(dir.as_str()));
    autorelease(env, dir)
}

fn NSTemporaryDirectory(env: &mut Environment) -> id {
    let dir = env.fs.home_directory().join("tmp");
    let dir = ns_string::from_rust_string(env, String::from(dir.as_str()));
    autorelease(env, dir)
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(NSHomeDirectory()),
    export_c_func!(NSTemporaryDirectory()),
    export_c_func!(NSSearchPathForDirectoriesInDomains(_, _, _)),
];

#[derive(Default)]
pub struct State {
    default_manager: Option<id>,
}

struct NSDirectoryEnumeratorHostObject {
    iterator: std::vec::IntoIter<GuestPathBuf>,
}
impl HostObject for NSDirectoryEnumeratorHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSFileManager: NSObject

+ (id)defaultManager {
    if let Some(existing) = env.framework_state.foundation.ns_file_manager.default_manager {
        existing
    } else {
        let new: id = msg![env; this new];
        env.framework_state.foundation.ns_file_manager.default_manager = Some(new);
        new
    }
}

- (id)currentDirectoryPath {
    ns_string::from_rust_string(env, env.fs.working_directory().as_str().to_string())
}

- (id)fileModificationDate {
    nil
}

- (bool)isReadableFileAtPath:(id)path { // NSString*
    let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
    log!("isReadableFileAtPath: {:?}", path);
    true
}

- (bool)changeCurrentDirectoryPath:(id)path {
    let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
    let path = GuestPath::new(&path);
    match env.fs.change_working_directory(path) {
        Ok(_) => true,
        Err(()) => false
    }
}

- (bool)fileExistsAtPath:(id)path { // NSString*
    if path == nil {
        return false
    }
    let res_exists = if path == nil {
        false
    } else {
        let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
        // fileExistsAtPath: will return true for directories
        // hence Fs::exists() rather than Fs::is_file() is appropriate.
        env.fs.exists(GuestPath::new(&path))
    };
    log_dbg!("[(NSFileManager*) {:?} fileExistsAtPath:{:?}] => {}", this, path, res_exists);
    res_exists
}

- (bool)fileExistsAtPath:(id)path // NSString*
             isDirectory:(MutPtr<bool>)is_dir {
    let (res_exists, res_is_dir) = if path == nil {
        (false, false)
    } else {
        // TODO: mutualize with fileExistsAtPath:
        let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
        let guest_path = GuestPath::new(&path);
        (env.fs.exists(guest_path), !env.fs.is_file(guest_path))
    };

    if !is_dir.is_null() {
        env.mem.write(is_dir, res_is_dir);
    }

    log_dbg!("[(NSFileManager*) {:?} fileExistsAtPath:{:?} isDirectory:{:?}] => {}", this, path, res_is_dir, res_exists);
    res_exists
}

- (bool)createFileAtPath:(id)path // NSString*
                contents:(id)data // NSData*
              attributes:(id)attributes { // NSDictionary*
    //assert!(attributes == nil); // TODO

    let path_str = ns_string::to_rust_string(env, path); // TODO: avoid copy
    // createFileAtPath: returns true if there's already a file at a given path.
    // If there's a directory, that's an error, though.
    if env.fs.is_file(GuestPath::new(&path_str)) {
        return true;
    }

    if data == nil {
        let empty: id = msg_class![env; NSData new];
        let res: bool = msg![env; empty writeToFile:path atomically:false];
        release(env, empty);
        res
    } else {
        msg![env; data writeToFile:path atomically:false]
    }
}

- (bool)removeItemAtPath:(id)path // NSString*
                   error:(MutPtr<id>)error { // NSError**
    let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
    match env.fs.remove(GuestPath::new(&path)) {
        Ok(()) => true,
        Err(()) => {
            if !error.is_null() {
                todo!(); // TODO: create an NSError if requested
            }
            false
        }
    }
}

- (bool)createDirectoryAtPath:(id)attributes // NSString *
  withIntermediateDirectories:(bool)createIntermediates
                   path:(id)path // NSDictionary*
                        error:(id)error { // NSError **
    assert!(attributes == nil); // TODO
    assert!(createIntermediates);

    let path_str = ns_string::to_rust_string(env, attributes); // TODO: avoid copy
    match env
        .fs
        .create_dir(GuestPath::new(&path_str))
    {
        Ok(()) => {
            log!("createDirectoryAtPath attributes {} => true", path_str);
            true
        }
        Err(()) => {
            log!(
                "Warning: createDirectoryAtPath attributes {} failed, returning false",
                path_str,
            );
            false
        }
    }
}

- (bool)createDirectoryAtPath:(id)path // NSString *
  withIntermediateDirectories:(bool)createIntermediates
                   attributes:(id)attributes // NSDictionary*
                        error:(id)error { // NSError **
    assert!(attributes == nil); // TODO
    assert!(createIntermediates);

    let path_str = ns_string::to_rust_string(env, path); // TODO: avoid copy
    match env
        .fs
        .create_dir(GuestPath::new(&path_str))
    {
        Ok(()) => {
            log!("createDirectoryAtPath path {} => true", path_str);
            true
        }
        Err(()) => {
            log!(
                "Warning: createDirectoryAtPath path {} failed, returning false",
                path_str,
            );
            false
        }
    }
}

- (id)enumeratorAtPath:(id)path { // NSString*
    let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
    let Ok(paths) = env.fs.enumerate_recursive(GuestPath::new(&path)) else {
        return nil;
    };
    let host_object = Box::new(NSDirectoryEnumeratorHostObject {
        iterator: paths.into_iter(),
    });
    let class = env.objc.get_known_class("NSDirectoryEnumerator", &mut env.mem);
    let enumerator = env.objc.alloc_object(class, host_object, &mut env.mem);
    autorelease(env, enumerator)
}

- (id)directoryContentsAtPath:(id)path /* NSString* */ { // NSArray*
    let path = ns_string::to_rust_string(env, path); // TODO: avoid copy
    let Ok(paths) = env.fs.enumerate(GuestPath::new(&path)) else {
        return nil;
    };
    let paths: Vec<GuestPathBuf> = paths
        .map(|path| GuestPathBuf::from(GuestPath::new(path)))
        .collect();
    log_dbg!("directoryContentsAtPath {}: {:?}", path, paths);
    let path_strings = paths
        .iter()
        .map(|name| ns_string::from_rust_string(env, name.as_str().to_string()))
        .collect();
    let res = ns_array::from_vec(env, path_strings);
    autorelease(env, res)
}

- (id)contentsOfDirectoryAtPath:(id)path /* NSString* */
                          error:(MutPtr<id>)error { // NSError**
    let contents: id = msg![env; this directoryContentsAtPath:path];
    if contents == nil && !error.is_null() {
        todo!(); // TODO: create an NSError if requested
    }
    contents
}

- (id)contentsAtPath:(id)path { // NSString *
    // TODO: return nil if path is directory
    // TODO: handle non-absolute paths?
    assert!(msg![env; path isAbsolutePath]);
    msg_class![env; NSData dataWithContentsOfFile:path]
}

- (bool)copyItemAtPath:(id)src // NSString*
                toPath:(id)dst // NSString*
                 error:(MutPtr<id>)_error { // NSError**
    let src = ns_string::to_rust_string(env, src);
    let dst = ns_string::to_rust_string(env, dst);
    let data = match env.fs.read(GuestPath::new(src.as_ref())) {
        Ok(d) => d,
        Err(_) => todo!()
    };
    if env.fs.write(GuestPath::new(dst.as_ref()), &data).is_err() {
        todo!();
    }
    true
}

- (())fileAttributesAtPath:(NSInteger)path traverseLink:(bool)_link {
    // TODO
}

- (id)attributesOfItemAtPath:(id)path // NSString*
                       error:(MutPtr<id>)error { // NSError**
    let path = if !path.is_null() { ns_string::to_rust_string(env, path) } else { "".into() };
    let (file_size) = match env.fs.open(GuestPath::new(path.as_ref())) {
        Ok(mut f) => {
            let file_size = f.seek(SeekFrom::End(0)).unwrap();

            (file_size)
        },
        Err(_) => (0),
    };
    let file_size_key: id = ns_string::get_static_str(env, "fileSize");
    let file_size_value: id = msg_class![env; NSNumber numberWithUnsignedLongLong:file_size];
    let dict = dict_from_keys_and_objects(env, &[(file_size_key, file_size_value)]);
    autorelease(env, dict)
}

@end

@implementation NSDirectoryEnumerator: NSEnumerator

- (id)nextObject {
    let host_obj = env.objc.borrow_mut::<NSDirectoryEnumeratorHostObject>(this);
    host_obj.iterator.next().map_or(nil, |s| ns_string::from_rust_string(env, String::from(s)))
}

@end

};
