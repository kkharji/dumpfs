/*!
 * XML writer implementation for DumpFS
 */

use std::fs::File;
use std::io::{self, BufWriter, Write};

use chrono::Local;
use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::config::Config;
use crate::git::GitHost;
use crate::types::{BinaryNode, DirectoryNode, FileNode, Metadata, Node, SymlinkNode};

/// XML writer for directory contents
pub struct XmlWriter {
    /// Writer configuration
    config: Config,
}

impl XmlWriter {
    /// Create a new XML writer
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Write the directory tree to an XML file
    pub fn write(&self, root_node: &DirectoryNode) -> io::Result<()> {
        let file = File::create(&self.config.output_file)?;
        let writer = BufWriter::new(file);
        let mut xml_writer = Writer::new_with_indent(writer, b' ', 2);

        // Write XML declaration
        xml_writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        // Start directory_scan element with timestamp
        let mut start_tag = BytesStart::new("directory_scan");
        let timestamp = Local::now().to_rfc3339();
        start_tag.push_attribute(("timestamp", timestamp.as_str()));
        xml_writer.write_event(Event::Start(start_tag))?;

        // Write system info
        self.write_system_info(&mut xml_writer)?;

        // Write repository structure summary
        self.write_overview(root_node, &mut xml_writer)?;

        // Write directory structure
        self.write_directory(root_node, &mut xml_writer)?;

        // End directory_scan element
        xml_writer.write_event(Event::End(BytesEnd::new("directory_scan")))?;

        Ok(())
    }

    /// Write system information to XML
    fn write_system_info<W: Write>(&self, writer: &mut Writer<W>) -> io::Result<()> {
        writer.write_event(Event::Start(BytesStart::new("system_info")))?;

        // Write hostname
        writer.write_event(Event::Start(BytesStart::new("hostname")))?;
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        writer.write_event(Event::Text(BytesText::new(&hostname)))?;
        writer.write_event(Event::End(BytesEnd::new("hostname")))?;

        // Write OS
        writer.write_event(Event::Start(BytesStart::new("os")))?;
        let os = std::env::consts::OS;
        writer.write_event(Event::Text(BytesText::new(os)))?;
        writer.write_event(Event::End(BytesEnd::new("os")))?;

        // Write kernel version
        writer.write_event(Event::Start(BytesStart::new("kernel")))?;
        let kernel = std::env::consts::FAMILY;
        writer.write_event(Event::Text(BytesText::new(kernel)))?;
        writer.write_event(Event::End(BytesEnd::new("kernel")))?;

        // Write Git repository information if available
        if let Some(git_repo) = &self.config.git_repo {
            writer.write_event(Event::Start(BytesStart::new("git_repository")))?;

            // Write URL
            writer.write_event(Event::Start(BytesStart::new("url")))?;
            writer.write_event(Event::Text(BytesText::new(&git_repo.url)))?;
            writer.write_event(Event::End(BytesEnd::new("url")))?;

            // Write host
            writer.write_event(Event::Start(BytesStart::new("host")))?;
            let host_name = match &git_repo.host {
                GitHost::GitHub => "github.com",
                GitHost::GitLab => "gitlab.com",
                GitHost::Bitbucket => "bitbucket.org",
                GitHost::Other(name) => name,
            };
            writer.write_event(Event::Text(BytesText::new(host_name)))?;
            writer.write_event(Event::End(BytesEnd::new("host")))?;

            // Write owner
            writer.write_event(Event::Start(BytesStart::new("owner")))?;
            writer.write_event(Event::Text(BytesText::new(&git_repo.owner)))?;
            writer.write_event(Event::End(BytesEnd::new("owner")))?;

            // Write repository name
            writer.write_event(Event::Start(BytesStart::new("name")))?;
            writer.write_event(Event::Text(BytesText::new(&git_repo.name)))?;
            writer.write_event(Event::End(BytesEnd::new("name")))?;

            writer.write_event(Event::End(BytesEnd::new("git_repository")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("system_info")))?;

        Ok(())
    }

    /// Write a directory node to XML
    fn write_directory<W: Write>(
        &self,
        dir: &DirectoryNode,
        writer: &mut Writer<W>,
    ) -> io::Result<()> {
        let mut start_tag = BytesStart::new("directory");
        start_tag.push_attribute(("name", dir.name.as_str()));
        start_tag.push_attribute(("path", dir.path.to_string_lossy().as_ref()));
        writer.write_event(Event::Start(start_tag))?;

        // Write metadata only if enabled
        if self.config.include_metadata {
            self.write_metadata(&dir.metadata, writer)?;
        }

        // Write contents
        writer.write_event(Event::Start(BytesStart::new("contents")))?;

        for node in &dir.contents {
            match node {
                Node::Directory(dir_node) => self.write_directory(dir_node, writer)?,
                Node::File(file_node) => self.write_file(file_node, writer)?,
                Node::Binary(bin_node) => self.write_binary(bin_node, writer)?,
                Node::Symlink(sym_node) => self.write_symlink(sym_node, writer)?,
            }
        }

        writer.write_event(Event::End(BytesEnd::new("contents")))?;
        writer.write_event(Event::End(BytesEnd::new("directory")))?;

        Ok(())
    }

    /// Write a file node to XML
    fn write_file<W: Write>(&self, file: &FileNode, writer: &mut Writer<W>) -> io::Result<()> {
        let mut start_tag = BytesStart::new("file");
        start_tag.push_attribute(("name", file.name.as_str()));
        start_tag.push_attribute(("path", file.path.to_string_lossy().as_ref()));
        writer.write_event(Event::Start(start_tag))?;

        // Write metadata only if enabled
        if self.config.include_metadata {
            self.write_metadata(&file.metadata, writer)?;
        }

        // Write content
        writer.write_event(Event::Start(BytesStart::new("content")))?;
        if let Some(content) = &file.content {
            // Use CDATA section to preserve formatting and avoid XML parsing issues
            writer.write_event(Event::CData(BytesCData::new(content)))?;
        }
        writer.write_event(Event::End(BytesEnd::new("content")))?;

        writer.write_event(Event::End(BytesEnd::new("file")))?;

        Ok(())
    }

    /// Write a binary file node to XML
    fn write_binary<W: Write>(
        &self,
        binary: &BinaryNode,
        writer: &mut Writer<W>,
    ) -> io::Result<()> {
        let mut start_tag = BytesStart::new("binary");
        start_tag.push_attribute(("name", binary.name.as_str()));
        start_tag.push_attribute(("path", binary.path.to_string_lossy().as_ref()));
        writer.write_event(Event::Start(start_tag))?;

        // Write metadata only if enabled
        if self.config.include_metadata {
            self.write_metadata(&binary.metadata, writer)?;
        }

        writer.write_event(Event::End(BytesEnd::new("binary")))?;

        Ok(())
    }

    /// Write a symlink node to XML
    fn write_symlink<W: Write>(
        &self,
        symlink: &SymlinkNode,
        writer: &mut Writer<W>,
    ) -> io::Result<()> {
        let mut start_tag = BytesStart::new("symlink");
        start_tag.push_attribute(("name", symlink.name.as_str()));
        start_tag.push_attribute(("path", symlink.path.to_string_lossy().as_ref()));
        writer.write_event(Event::Start(start_tag))?;

        // Write metadata only if enabled
        if self.config.include_metadata {
            self.write_metadata(&symlink.metadata, writer)?;
        }

        // Write target
        writer.write_event(Event::Start(BytesStart::new("target")))?;
        writer.write_event(Event::Text(BytesText::new(&symlink.target)))?;
        writer.write_event(Event::End(BytesEnd::new("target")))?;

        writer.write_event(Event::End(BytesEnd::new("symlink")))?;

        Ok(())
    }

    /// Write repository structure to XML
    fn write_overview<W: Write>(
        &self,
        root_node: &DirectoryNode,
        writer: &mut Writer<W>,
    ) -> io::Result<()> {
        writer.write_event(Event::Start(BytesStart::new("overview")))?;

        // Recursively write the directory structure with only names
        Self::write_node_overview(root_node, writer)?;

        writer.write_event(Event::End(BytesEnd::new("overview")))?;

        Ok(())
    }

    /// Write a hierarchical structure node with only names
    fn write_node_overview<W: Write>(
        dir: &DirectoryNode,
        writer: &mut Writer<W>,
    ) -> io::Result<()> {
        // Create a directory element with only the name
        let mut start_tag = BytesStart::new("directory");
        start_tag.push_attribute(("name", dir.name.as_str()));
        writer.write_event(Event::Start(start_tag))?;

        // Write child elements (files and directories)
        for node in &dir.contents {
            match node {
                Node::Directory(dir_node) => {
                    Self::write_node_overview(dir_node, writer)?;
                }
                Node::File(file_node) => {
                    let mut file_tag = BytesStart::new("file");
                    file_tag.push_attribute(("name", file_node.name.as_str()));
                    writer.write_event(Event::Empty(file_tag))?;
                }
                Node::Binary(bin_node) => {
                    let mut bin_tag = BytesStart::new("file");
                    bin_tag.push_attribute(("name", bin_node.name.as_str()));
                    writer.write_event(Event::Empty(bin_tag))?;
                }
                Node::Symlink(symlink_node) => {
                    let mut link_tag = BytesStart::new("symlink");
                    link_tag.push_attribute(("name", symlink_node.name.as_str()));
                    writer.write_event(Event::Empty(link_tag))?;
                }
            }
        }

        writer.write_event(Event::End(BytesEnd::new("directory")))?;

        Ok(())
    }

    /// Write metadata to XML
    fn write_metadata<W: Write>(
        &self,
        metadata: &Metadata,
        writer: &mut Writer<W>,
    ) -> io::Result<()> {
        writer.write_event(Event::Start(BytesStart::new("metadata")))?;

        // Write size
        writer.write_event(Event::Start(BytesStart::new("size")))?;
        writer.write_event(Event::Text(BytesText::new(&metadata.size.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("size")))?;

        // Write modified time
        writer.write_event(Event::Start(BytesStart::new("modified")))?;
        let modified = chrono::DateTime::<chrono::Local>::from(metadata.modified).to_rfc3339();
        writer.write_event(Event::Text(BytesText::new(&modified)))?;
        writer.write_event(Event::End(BytesEnd::new("modified")))?;

        // Write permissions
        writer.write_event(Event::Start(BytesStart::new("permissions")))?;
        writer.write_event(Event::Text(BytesText::new(&metadata.permissions)))?;
        writer.write_event(Event::End(BytesEnd::new("permissions")))?;

        writer.write_event(Event::End(BytesEnd::new("metadata")))?;

        Ok(())
    }
}
