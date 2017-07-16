//! # iron-send-file
//!
//! Serve files with Range header support for Iron library.

#[macro_use]
extern crate iron;
extern crate lazy_static;
extern crate hyper;
extern crate conduit_mime_types as mime_types;
extern crate http_range;

use std::str;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use iron::{IronResult, Request, Response, Set};
use iron::status;
use iron::headers;
use http_range::{HttpRange, HttpRangeParseError};

/// Send file
///
/// Request is needed for `Range` header.
/// Response parameter allows setting custom headers for response.
/// Path is path of the file to be served.
pub fn send_file(req: &Request, mut res: Response, path: &Path) -> IronResult<Response> {
    let mut file = itry!(File::open(path), (status::NotFound, "Not Found"));
    let size = itry!(file.metadata(),
                     (status::InternalServerError, "Internal server error"))
        .len();

    let range = match req.headers.get_raw("Range") {
        Some(range) => {
            let range_str = itry!(str::from_utf8(&range[0]),
                                  (status::BadRequest, "Invalid Range header"));
            let mut ranges = match HttpRange::parse(range_str, size) {
                Ok(r) => r,
                Err(err) => {
                    match err {
                        HttpRangeParseError::NoOverlap => {
                            res.headers
                                .set(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                                    range: None,
                                    instance_length: Some(size),
                                }))
                        }
                        HttpRangeParseError::InvalidRange => (),
                    }

                    return Ok(res.set((status::RangeNotSatisfiable, "Invalid range")));
                }
            };

            match ranges.len() {
                0 => None,
                1 => Some(ranges.remove(0)),
                _ => return Ok(res.set((status::BadRequest, "Multiple ranges not supported"))),
            }
        }
        None => None,
    };

    res.set_mut(path);

    match range {
        Some(range) => {
            res.status = Some(status::PartialContent);

            res.headers.set(headers::ContentLength(range.length));

            res.headers.set(headers::ContentRange(headers::ContentRangeSpec::Bytes {
                range: Some((range.start, range.start + range.length - 1)),
                instance_length: Some(size),
            }));

            let _ = file.seek(SeekFrom::Start(range.start));

            let take = file.take(range.length);

            res.body = Some(Box::new(Box::new(take) as Box<Read + Send>));
        }
        None => {
            res.status = Some(status::Ok);

            res.headers.set(headers::ContentLength(size));

            res.body = Some(Box::new(file));
        }
    }

    Ok(res)
}
