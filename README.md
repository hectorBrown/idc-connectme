# idc-connectme

This is a program to connect to captive portal WiFi networks automatically.

It uses a webdriver to open a headless browser, navigate to the captive portal
URL, and click an `<input type="submit">` element to log in.

It is currently not able to handle more complex captive portals that require
checkboxes, or emails, or captchas. Hopefully it will be able to do some of
these things in the future.

If it fails, it simply presents the portal page to the user to log in manually.

## Installation

First install `chromedriver` then clone this repo and `cargo install --path .`.
The program will be installed to `~/.cargo/bin/idc-connectme`.

## Usage

You can do:

```shell
idc-connectme "$(ip --oneline route get 1.1.1.1 | awk '{print $3'})"
```

Or you can run it with a `NetworkManager` dispatcher script. Create a file at
`/etc/NetworkManager/dispatcher.d/90-idc-connectme` with the following content
(modified from
[captive-portal.sh](https://github.com/Seme4eg/captive-portal-sh)):

```shell
#!/bin/sh -e

# man 8 NetworkManager-dispatcher

PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

if [ -x "/usr/bin/logger" ]; then
  logger="/usr/bin/logger -s -t captive-portal"
else
  logger=":"
fi

open_captive() {
  captive_url=http://$(ip --oneline route get 1.1.1.1 | awk '{print $3}')
  sudo -u "$1" DISPLAY=":0" \
    DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/"$(id -u "$1")"/bus \
    /home/[YOUR_USERNAME]/.cargo/bin/idc-connectme -u "$1" "${captive_url}"
}

case "$2" in
  connectivity-change)
    $logger -p user.debug \
      "dispatcher script triggered on connectivity change: $CONNECTIVITY_STATE"

    if [ "$CONNECTIVITY_STATE" = "PORTAL" ]; then
      user=$(who | head -n1 | cut -d' ' -f 1)
      while [ -z $user ]; do
      user=$(who | head -n1 | cut -d' ' -f 1)
      sleep 0.5
      done

      $logger "Running browser as '$user' to login in captive portal"

      open_captive "$user" || $logger -p user.err "Failed for user: '$user'"
    fi
    ;;
  *) exit 0 ;;
esac
```
