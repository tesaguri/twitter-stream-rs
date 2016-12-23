use futures::{Async, Future, Poll, Sink, Stream};
use futures::stream;
use futures::sync::mpsc::{self, Receiver, Sender};
use std::io::BufRead;
use std::thread;
use std::time::{Duration, Instant};
use {Error, Result};

/// A stream over each non-empty line on a `BufRead`.
pub struct Lines {
    rx: Receiver<Result<String>>,
    timer: Instant,
}

/// Adds to `Sender` an ability to send an `Error` to its corresponding `Receiver` while panicking.
struct SenderPanicGuard(Option<Sender<Result<String>>>);

/// Returns a stream over each non-empty line on `a`.
#[allow(unused_variables)]
pub fn lines<A: BufRead + Send + 'static>(a: A) -> Lines {
    let (tx, rx) = mpsc::channel(8);

    thread::Builder::new().name("twitter_user_stream_sender".into()).spawn(move || {
        let txg = SenderPanicGuard(Some(tx.clone()));
        let stream = stream::iter(a.lines().map(|r| Ok(r.map_err(Error::from))));
        info!("starting to listen on a stream");
        tx.send_all(stream).wait().unwrap();
    }).unwrap();

    Lines {
        rx: rx,
        timer: Instant::now(),
    }
}

impl Stream for Lines {
    type Item = String;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<String>, Error> {
        loop {
            match self.rx.poll().expect("Receiver failed") {
                Async::Ready(Some(line)) => {
                    info!("duration since last message: {}", {
                        let elapsed = self.timer.elapsed();
                        elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000f64
                    });
                    self.timer = Instant::now();
                    let line = line?;
                    if !line.is_empty() {
                        return Ok(Some(line).into());
                    }
                },
                Async::Ready(None) => return Ok(None.into()),
                Async::NotReady => {
                    if self.timer.elapsed() < Duration::from_secs(95) {
                        return Ok(Async::NotReady);
                    } else {
                        return Err(Error::TimedOut.into());
                    }
                },
            }
        }
    }
}

impl Drop for SenderPanicGuard {
    #[allow(unused_must_use)]
    fn drop(&mut self) {
        if thread::panicking() {
            if let Some(tx) = self.0.take() {
                tx.send(Err(Error::InternalPanicError)).wait();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::*;

    #[test]
    fn lines_test() {
        let input = b"\
            {\"friends\":[123456789,234567890,345678901,4567890123,567890123,678901234]}






\
            {\"event\":\"favorite\",\"created_at\":\"Sun Jan 01 00:00:00 +0000 2017\",\"source\":{\"id\":1234567890,\"id_str\":\"1234567890\",\"name\":\"test\",\"screen_name\":\"twitter_test\",\"location\":\"somewhere\",\"url\":null,\"description\":\"test\",\"protected\":false,\"followers_count\":123456789,\"friends_count\":123,\"listed_count\":12,\"created_at\":\"Sat Dec 01 00:00:00 +0000 2016\",\"favourites_count\":1,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":1234,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"000000\",\"profile_background_image_url\":\"http:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_image_url_https\":\"https:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_banner_url\":null,\"profile_link_color\":\"000000\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":false,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"regular\"},\"target\":{\"id\":2345678901,\"id_str\":\"2345678901\",\"name\":\"Test\",\"screen_name\":\"test_twitter\",\"location\":\"Twitter\",\"url\":\"https://example.com/\",\"description\":\"Test\",\"protected\":false,\"followers_count\":12,\"friends_count\":1,\"listed_count\":0,\"created_at\":\"Fri Jan 01 00:00:00 +0000 2016\",\"favourites_count\":1234,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":12,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"FFFFFF\",\"profile_background_image_url\":\"http://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_image_url_https\":\"https://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_banner_url\":\"https:\\/\\/pbs.twimg.com\\/profile_banners\\/6253282\\/1347394302\",\"profile_link_color\":\"FFFFFF\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":true,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"none\"}}
\
            {\"event\":\"unfavorite\",\"created_at\":\"Sun Jan 01 00:00:00 +0000 2017\",\"source\":{\"id\":1234567890,\"id_str\":\"1234567890\",\"name\":\"test\",\"screen_name\":\"twitter_test\",\"location\":\"somewhere\",\"url\":null,\"description\":\"test\",\"protected\":false,\"followers_count\":123456789,\"friends_count\":123,\"listed_count\":12,\"created_at\":\"Sat Dec 01 00:00:00 +0000 2016\",\"favourites_count\":1,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":1234,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"000000\",\"profile_background_image_url\":\"http:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_image_url_https\":\"https:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_banner_url\":null,\"profile_link_color\":\"000000\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":false,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"regular\"},\"target\":{\"id\":2345678901,\"id_str\":\"2345678901\",\"name\":\"Test\",\"screen_name\":\"test_twitter\",\"location\":\"Twitter\",\"url\":\"https://example.com/\",\"description\":\"Test\",\"protected\":false,\"followers_count\":12,\"friends_count\":1,\"listed_count\":0,\"created_at\":\"Fri Jan 01 00:00:00 +0000 2016\",\"favourites_count\":1234,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":12,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"FFFFFF\",\"profile_background_image_url\":\"http://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_image_url_https\":\"https://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_banner_url\":\"https:\\/\\/pbs.twimg.com\\/profile_banners\\/6253282\\/1347394302\",\"profile_link_color\":\"FFFFFF\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":true,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"none\"}}

\
            ";

        let mut iter = lines(Cursor::new(input.to_vec())).wait();

        assert_eq!(
            &iter.next().unwrap().unwrap(),
            "{\"friends\":[123456789,234567890,345678901,4567890123,567890123,678901234]}"
        );
        assert_eq!(
            &iter.next().unwrap().unwrap(),
            "{\"event\":\"favorite\",\"created_at\":\"Sun Jan 01 00:00:00 +0000 2017\",\"source\":{\"id\":1234567890,\"id_str\":\"1234567890\",\"name\":\"test\",\"screen_name\":\"twitter_test\",\"location\":\"somewhere\",\"url\":null,\"description\":\"test\",\"protected\":false,\"followers_count\":123456789,\"friends_count\":123,\"listed_count\":12,\"created_at\":\"Sat Dec 01 00:00:00 +0000 2016\",\"favourites_count\":1,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":1234,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"000000\",\"profile_background_image_url\":\"http:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_image_url_https\":\"https:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_banner_url\":null,\"profile_link_color\":\"000000\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":false,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"regular\"},\"target\":{\"id\":2345678901,\"id_str\":\"2345678901\",\"name\":\"Test\",\"screen_name\":\"test_twitter\",\"location\":\"Twitter\",\"url\":\"https://example.com/\",\"description\":\"Test\",\"protected\":false,\"followers_count\":12,\"friends_count\":1,\"listed_count\":0,\"created_at\":\"Fri Jan 01 00:00:00 +0000 2016\",\"favourites_count\":1234,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":12,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"FFFFFF\",\"profile_background_image_url\":\"http://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_image_url_https\":\"https://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_banner_url\":\"https:\\/\\/pbs.twimg.com\\/profile_banners\\/6253282\\/1347394302\",\"profile_link_color\":\"FFFFFF\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":true,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"none\"}}"
        );
        assert_eq!(
            &iter.next().unwrap().unwrap(),
            "{\"event\":\"unfavorite\",\"created_at\":\"Sun Jan 01 00:00:00 +0000 2017\",\"source\":{\"id\":1234567890,\"id_str\":\"1234567890\",\"name\":\"test\",\"screen_name\":\"twitter_test\",\"location\":\"somewhere\",\"url\":null,\"description\":\"test\",\"protected\":false,\"followers_count\":123456789,\"friends_count\":123,\"listed_count\":12,\"created_at\":\"Sat Dec 01 00:00:00 +0000 2016\",\"favourites_count\":1,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":1234,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"000000\",\"profile_background_image_url\":\"http:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_image_url_https\":\"https:\\/\\/abs.twimg.com\\/images\\/themes\\/theme1\\/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_0_normal.png\",\"profile_banner_url\":null,\"profile_link_color\":\"000000\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":false,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"regular\"},\"target\":{\"id\":2345678901,\"id_str\":\"2345678901\",\"name\":\"Test\",\"screen_name\":\"test_twitter\",\"location\":\"Twitter\",\"url\":\"https://example.com/\",\"description\":\"Test\",\"protected\":false,\"followers_count\":12,\"friends_count\":1,\"listed_count\":0,\"created_at\":\"Fri Jan 01 00:00:00 +0000 2016\",\"favourites_count\":1234,\"utc_offset\":00000,\"time_zone\":\"London\",\"geo_enabled\":false,\"verified\":false,\"statuses_count\":12,\"lang\":\"en\",\"contributors_enabled\":false,\"is_translator\":false,\"is_translation_enabled\":false,\"profile_background_color\":\"FFFFFF\",\"profile_background_image_url\":\"http://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_image_url_https\":\"https://abs.twimg.com/images/themes//theme1/bg.png\",\"profile_background_tile\":false,\"profile_image_url\":\"http:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_image_url_https\":\"https:\\/\\/abs.twimg.com\\/sticky\\/default_profile_images\\/default_profile_1_normal.png\",\"profile_banner_url\":\"https:\\/\\/pbs.twimg.com\\/profile_banners\\/6253282\\/1347394302\",\"profile_link_color\":\"FFFFFF\",\"profile_sidebar_border_color\":\"000000\",\"profile_sidebar_fill_color\":\"000000\",\"profile_text_color\":\"000000\",\"profile_use_background_image\":true,\"default_profile\":false,\"default_profile_image\":true,\"following\":null,\"follow_request_sent\":null,\"notifications\":null,\"translator_type\":\"none\"}}"
        );
        assert!(iter.next().is_none());
    }
}
