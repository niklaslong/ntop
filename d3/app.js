var svg = d3.select("svg"),
    width = +svg.attr("width"),
    height = +svg.attr("height");

svg.append("rect")
    .attr("width", "100%")
    .attr("height", "100%")
    .attr("fill", "black");

var data = { "vertices": [], "edges": [] };
var nodes = data["vertices"];
var links = data["edges"];

const simulation = d3.forceSimulation().on("tick", ticked);

var link = svg.append("g")
    .attr("stroke", "#999")
    .attr("stroke-opacity", 0.6)
    .selectAll("line");

var node = svg.append("g")
    .attr("stroke", "#fff")
    .attr("stroke-width", 1.5)
    .selectAll("circle");

restart();

function restart() {
    d3.timeout(function() {

        postData('http://localhost:3030', { "jsonrpc": "2.0", "method": "graph", "id": 123 })
            .then(data => {
                console.log(JSON.stringify(data));
                merge_data(data);
                update_graph();
                restart();
            });
    }, 100);
}

function merge_data(data) {

    var added_nodes = data["result"]["added_vertices"];
    var removed_nodes = data["result"]["removed_vertices"];

    removed_nodes.forEach(removed_node => {
        const isRemoved = (node) => node.id == removed_node.id;
        var i = nodes.findIndex(isRemoved);

        nodes.splice(i, 1);
    });

    added_nodes.forEach(added_node => {
        nodes.push(added_node)
    });

    var added_links = data["result"]["added_edges"];
    var removed_links = data["result"]["removed_edges"];

    removed_links.forEach(removed_link => {
        const isRemoved = (link) => link.source.id == removed_link.source && link.target.id == removed_link.target;
        var i = links.findIndex(isRemoved);
        console.log(i);
        console.log(links[i]);
        console.log(links.splice(i, 1));
    });

    added_links.forEach(added_link => {
        links.push(added_link)
    });

    // console.log(JSON.stringify(links));

    // console.log(nodes);
    // links = data["result"]["edges"];
    // nodes = data["result"]["vertices"];
}

function update_graph() {

    // Apply the general update pattern to the nodes.
    node = node.data(nodes, d => d.id).join(
        enter => enter.append("circle").attr("fill", d => {
            if (d.is_bootnode) { return "red" } else { return "green" }
        }).attr("r", 5),
        update => update,
        exit => exit.remove()
    );

    link = link.data(links, d => d.source.id + "-" + d.target.id).join(
        enter => enter.append("line")
        .attr("stroke-width", .8),
        update => update,
        exit => exit.remove()
    );

    // Update and restart the simulation.
    simulation.nodes(nodes);
    simulation.force("link", d3.forceLink(links).id(d => d.id).distance(50))
        .force("charge", d3.forceManyBody().strength(-100))
        // .force("center", d3.forceCenter(width / 2, height / 2));
        .force("x", d3.forceX(width / 2).strength(.05))
        .force("y", d3.forceY(height / 2).strength(.05));
    simulation.alpha(.7).restart();
}

function ticked() {
    link
        .attr("x1", d => d.source.x)
        .attr("y1", d => d.source.y)
        .attr("x2", d => d.target.x)
        .attr("y2", d => d.target.y);

    node
        .attr("cx", d => d.x)
        .attr("cy", d => d.y);
}

async function postData(url = '', data = {}) {
    const response = await fetch(url, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify(data)
    });
    return response.json();
}