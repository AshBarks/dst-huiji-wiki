from mwclient import Site

site = Site('dontstarve.huijiwiki.com', custom_headers={
            'X-authkey': config["huijiwiki"]["X-authkey"]})
site.login(
    username = config["huijiwiki"]["username"],
    password = config["huijiwiki"]["password"],
)
