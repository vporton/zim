# Zim extractor


## Build

```sh
> cargo build --release
```

## Usage

To add a file `data.zim` to ipfs do the following.


```sh
> ./target/release/extract_zim --skip-link data.zim
> ipfs add -r out
> ipfs files cp /ipfs/<outhash> /
> ipfs files mv /<outhash> /data
> ./target/release/ipfs_link /data data.zim
```

and then execute all commands in `link.txt`
